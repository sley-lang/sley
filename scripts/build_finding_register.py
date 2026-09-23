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
CONTRACT_REVISION = 8
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


def contract_revision_round(field: str) -> int | None:
    """The contract revision a `<lane>_..._revision_<N>` round names, or None."""
    match = re.search(r"_revision_(\d+)$", field)
    return int(match.group(1)) if match else None


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


def round_scope(note: object) -> str | None:
    """The `on <sha40>` scope a round's `_note` records, if any."""
    if not isinstance(note, str):
        return None
    found = re.search(r"\bon ([0-9a-f]{40})\b", note)
    return found.group(1) if found else None


def scoped_before(pass_scope: str | None, fail_scope: str | None) -> bool:
    """Whether the notes' scopes show the PASS was filed at an ancestor of
    the FAIL round's commit: git ancestry orders same-day rounds exactly
    (Nabu P3 at 6589c6ec: a REVISE filed later the same day had folded
    under the ancestor PASS by the day-granular date rule). A PASS whose
    scope is not strictly later than the FAIL's never folds it."""
    if not pass_scope or not fail_scope:
        return False
    return not strictly_later_scope(pass_scope, fail_scope)


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
    # Two numbered contract-revision rounds of one subject (`..._revision_9`
    # REVISE, `..._revision_10` PASS): the later revision's PASS closes the
    # earlier round, never the reverse. Both carry the early token
    # `revision`, so the token rule below would leave every earlier
    # revision round open forever and no revised package could complete.
    # The caller still refuses a PASS dated or scoped before the round.
    pass_round = contract_revision_round(pass_field)
    fail_round = contract_revision_round(fail_field)
    if pass_round is not None and fail_round is not None:
        return pass_core == fail_core and pass_round > fail_round
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
                            "round_scope": round_scope(node.get(f"{field}_note")),
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
    scopes: dict[tuple[str, str], str | None] = {}
    for item in obligations:
        dates[(item["section"], item["field"])] = item["round_date"]
        scopes[(item["section"], item["field"])] = item["round_scope"]
        if item["state"] == "PASS" and item["reviewer"] is not None:
            passes.setdefault((item["section"], item["reviewer"]), []).append(item["field"])
    for item in obligations:
        if item["state"] == "FAIL_ROUND":
            superseder = None
            if item["reviewer"] is not None:
                for candidate in passes.get((item["section"], item["reviewer"]), []):
                    if supersedes(candidate, item["field"]) and not dated_before(
                        dates.get((item["section"], candidate)), item["round_date"]
                    ) and not scoped_before(scopes.get((item["section"], candidate)), item["round_scope"]):
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


NOTE_SCOPE = re.compile(r"\bon ([0-9a-f]{40})\b")


_generation: dict[str, int] = {}


def scope_generation(short: str) -> int:
    """The commit's ancestor count (orders scopes by ancestry; -1 unknown)."""
    if short not in _generation:
        import subprocess

        done = subprocess.run(["git", "rev-list", "--count", short], cwd=ROOT, capture_output=True, text=True, check=False)
        _generation[short] = int(done.stdout.strip()) if done.returncode == 0 and done.stdout.strip().isdigit() else -1
    return _generation[short]


_raising: dict[tuple[str, str], str | None] = {}


def raising_scope(section: str, claim: str, fields: dict | None = None) -> str | None:
    """The scope the claim was raised at: its tag; else the earliest filed
    transcript of its lane (or, lane-less, of its field) whose `[Pn]`
    finding line begins with the claim's description; else, for a frozen
    revision field or a closure-note field, the `on <sha>` of that field's
    note (Vulcan P3 at 79fdcc63: untagged claims had no scope relation, so
    an own-round line could retire them)."""
    match = CLAIM_TAG.match(claim)
    if match and match.group(2):
        return match.group(2)[:7]
    key = (section, claim)
    if key in _raising:
        return _raising[key]
    prefix = match.group(1) if match else claim.split(":", 1)[0]
    lane = reviewer_of(prefix)
    description = (claim.split(": ", 1)[1] if ": " in claim else "").strip()
    directory = ROOT / "evidence/review/verdicts" / section.split(".")[0]
    found: list[str] = []
    if description and directory.is_dir():
        paths = []
        for path in sorted(directory.glob("*.md")):
            stamp = re.search(r"-([0-9a-f]{7,40})\.md$", path.name)
            if not stamp or not is_tracked(path.relative_to(ROOT).as_posix()):
                continue
            if lane and not path.name.startswith(lane):
                continue
            if not lane and not path.name.startswith(prefix.removesuffix("_note")):
                continue
            paths.append((path, stamp.group(1)[:7]))
        # The full stored description first (Nabu P4 at b58ac1e0: an
        # 80-character prefix had collided transcripts differing past it);
        # the prefix only for older truncated rounds. Carry decorations are
        # stripped: the raising line recorded the original prose.
        for text in (_strip_carry(description), description[:80]):
            for path, stamp in paths:
                for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                    item = re.match(r"^(?:FINDINGS:\s*)?\[(P[0-4])\]\s*(.*)$", line)
                    if item and _strip_carry(item.group(2).strip()).startswith(text):
                        found.append(stamp)
                        break
            if found:
                break
    scope: str | None = None
    if found:
        scope = min(found, key=scope_generation)
    elif fields and (re.search(r"_revision_[0-9]+$", prefix) or prefix.endswith("_closure_note")):
        note = fields.get(prefix + "_note") if not prefix.endswith("_closure_note") else fields.get(prefix)
        noted = NOTE_SCOPE.search(note) if isinstance(note, str) else None
        scope = noted.group(1)[:7] if noted else None
    _raising[key] = scope
    return scope


def _strip_carry(description: str) -> str:
    """The claim's description without its carry decorations: a re-statement
    appends its `carried from <sha>` root and status to the original prose,
    while the raising transcript recorded the original (Nabu P4 at b58ac1e0:
    the carry tail had blocked the raising-transcript match). Only the
    carry clause itself is removed — never the text around it (a trailing
    `OPEN.*` cut had eaten the distinguishing path and false-matched a
    same-kind finding)."""
    text = re.sub(r"\(\s*carr(?:ied|y)\s+(?:over\s+)?from\s+[0-9a-f]{7,40}[^)]*\)", "", description)
    text = CARRIED_FROM.sub("", text)
    text = LEADING_CARRY.sub("", text)
    text = re.sub(r"\s*[—–\-:,]\s*\bOPEN\b\s*(\([^)]*\))?\s*$", "", text)
    return re.sub(r"[\s\-–—,;:.()]+$", "", text).strip()


def _finding_line_match(lines: list[str], description: str) -> str | None:
    """The `[Pn]` severity of the finding line beginning with the claim's
    description: the full stored description first, the 80-character prefix
    only for older rounds that recorded truncated descriptions (Nabu P4 at
    b58ac1e0: prefix-only matching had collided two transcripts differing
    past character 80). Both sides are carry-stripped: the raising line of
    a carried finding carries the same decorations."""
    full = _strip_carry(description.strip())
    for line in lines:
        found = re.match(r"^(?:FINDINGS:\s*)?\[(P[0-4])\]\s*(.*)$", line)
        if found and _strip_carry(found.group(2).strip()).startswith(full):
            return found.group(1)
    prefix = description.strip()[:80]
    for line in lines:
        found = re.match(r"^(?:FINDINGS:\s*)?\[(P[0-4])\]\s*(.*)$", line)
        if found and found.group(2).strip().startswith(prefix):
            return found.group(1)
    return None


def raising_severity(section: str, claim: str) -> str | None:
    """The severity the claim's raising transcript records for it: the
    `[Pn]` finding line of `<section>/<lane>*-<scope7>.md` whose text begins
    with the claim's description, or None when the claim is untagged, the
    transcript is not filed, or no finding line matches (older rounds
    recorded truncated descriptions)."""
    match = CLAIM_TAG.match(claim)
    if not match or not match.group(2):
        return None
    lane = reviewer_of(match.group(1))
    description = claim.split(": ", 1)[1] if ": " in claim else ""
    if not lane or not description:
        return None
    directory = ROOT / "evidence/review/verdicts" / section.split(".")[0]
    for path in sorted(directory.glob(f"{lane}*-{match.group(2)[:7]}.md")) if directory.is_dir() else []:
        if not is_tracked(path.relative_to(ROOT).as_posix()):
            continue
        hit = _finding_line_match(path.read_text(encoding="utf-8", errors="replace").splitlines(), description)
        if hit:
            return hit
    return None


def package_open_claims(summary: dict) -> dict[str, int]:
    """Every per-package open claim, dotted field to open count. A claim's
    severity is bound to the `[Pn]` line its raising transcript records for
    it (Vulcan P3 at 6589c6ec: severity had been list membership only)."""
    claims: dict[str, int] = {}
    for section, value in summary.items():
        if not isinstance(value, dict):
            continue
        for field, item in value.items():
            if PACKAGE_OPEN_LIST.match(field) and isinstance(item, list):
                # An open list without its count is refused (Vulcan P4 at
                # 8966da2e: the GA row sums the count keys, so a missing
                # count had undercounted silently).
                count = value.get(f"{field}_count")
                if not isinstance(count, int) or count < 0:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field} has no matching {field}_count",
                    )
                for claim in item:
                    raised = raising_severity(section, claim) if isinstance(claim, str) else None
                    if raised and raised != "P" + field[1]:
                        raise RegisterError(
                            RegisterErrorCode.SUMMARY_INVALID,
                            f"{section}.{field}: the raising transcript records {raised}: {claim[:60]!r}",
                        )
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
# `STILL`/`still`/`remains OPEN` reads OPEN (Vulcan P4 at 8966da2e: thirteen
# live `— STILL OPEN` finding lines had been invisible to the OPEN check);
# a bare `open` never counts, and `reopened` prose is not a status.
STATUS_OPEN = re.compile(STATUS_MARK + r"(?:(?:STILL|still|remains)\s+)?(?:OPEN|Open)\b")
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
        # A paren qualifier naming open (any case) refuses the cell
        # (Vulcan P4 at 8966da2e: `**CLOSED (leg 2 open)**` had counted as
        # an own status through the uppercase-only lookahead).
        if re.fullmatch(r"(?:Both|All\w*|Each)?(?:CLOSED|OPEN|Open)(?:\((?![^)]*[Oo][Pp][Ee][Nn])[^)]{0,24}\))?\.?", cell):
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
    return severity in item_severities(line[: closed[0].start()])


SEVERITY_RUN = re.compile(r"(?<![A-Z0-9])P[0-4](?![0-9])(?:\s*(?:/|,|and|&|\+)\s*P[0-4](?![0-9]))*")


def item_severities(head: str) -> set[str]:
    """The severities the item's own leading token names: the first severity
    run of the head (`[P1]`, `Prior P3`, `P2/P3`, `P3, P4 and P2`), never a
    severity mentioned later in the head's prose (Vulcan P3 at 1a9f0aab: a
    `[P3] record-provenance …` line had been citable for a P2 it discussed)."""
    run = SEVERITY_RUN.search(head)
    return set(re.findall(r"P[0-4]", run.group(0))) if run else set()


def is_open_line(line: str, severity: str) -> bool:
    """The line records `severity` OPEN: an item status line (`| P3 | **OPEN**
    |`, `- **[P3] … — OPEN.**`) or a finding-raising `[P3] …` line carrying
    an unquoted OPEN status, whose own leading severity is `severity`."""
    if line.startswith(("VERDICT:", "SUMMARY:", "FINDINGS:")):
        return False
    if FINDING_LINE.match(line):
        # A finding-raising line states its status as a bare word standing
        # as one (`… - prior item 10 OPEN (advisory): …`); prose merely
        # mentioning OPEN (`OPEN refusal`) is not a status (Nabu P4 at
        # b58ac1e0).
        bare = [
            m for m in re.finditer(r"\bOPEN\b(?=\s*(?:\(|:|$))", line)
            if not _quoted(line, m.start(), m.end())
        ]
        return bool(bare) and line.startswith(f"[{severity}]")
    opens = [m for m in STATUS_OPEN.finditer(line) if not _quoted(line, m.start(), m.end())]
    if not opens:
        return False
    own = [m for m in opens if _own_status(line, m)]
    return bool(own) and severity in item_severities(line[: own[0].start()])


def kind_phrase_of(claim: str) -> str:
    """The claim's bracketed kind, lowered (`[record-note]` → `record-note`)."""
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    kind = re.match(r"\[([^\]]+)\]", body)
    return kind.group(1).strip().lower() if kind else ""


def claim_finding_ids(body: str) -> set[str]:
    """Finding ids the claim's leading text names (`V-02`, `RW090-DEV-01`,
    `S20-700-PACK-001`): never a document id (`ADR-0040`, `S20-540`) and
    never a number carried by a cited path (Ariadne/Nabu P3 at 79fdcc63;
    Ariadne P4 at 8966da2e: digit-led middle segments such as `700` had
    split the id, so `S20-700-PACK-001` read as `PACK-001`)."""
    leading = body[:200].split(";")[0]
    path_text = " ".join(PATH_TOKEN.findall(body))
    return {
        fid for fid in re.findall(r"\b[A-Z](?:[A-Z0-9]+-)+[0-9]{2,}\b|\b[A-Z]{1,3}-[0-9]{2}\b", leading)
        if not re.fullmatch(r"(?:ADR|S20|RW|T|S)[0-9]*-[0-9]+", fid) and fid not in path_text
    }


def identity_tokens(claim: str) -> tuple[set[str], set[str]]:
    """(kind phrases, identifiers and finding ids) of a claim, as the
    relation reads them."""
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    tags = [tag.lower() for tag in CATEGORY.findall(body[:120])]
    phrases = {phrase.strip() for tag in tags for phrase in re.split(r"[/—,;:]", tag) if phrase.strip() and phrase.strip() not in STOP_WORDS}
    path_text = " ".join(PATH_TOKEN.findall(body)).lower()
    identifiers = {
        word for word in content_words(body)
        if "_" in word and len(word) >= 8 and word not in path_text and not is_lane_field_name(word)
    }
    return phrases, identifiers | claim_finding_ids(body)


def shared_vocabulary(section: dict, severity: str, claim: str, section_name: str | None = None) -> set[str]:
    """Kind phrases and identifiers this claim shares with another claim of
    the same lane and severity in the section (open, retired or re-stated):
    shared vocabulary is not an identity (Ariadne P2 / Vulcan P3 at
    6589c6ec: one closure line's kind matched three open claims of its lane;
    a proof-record field name named five)."""
    prefix = CLAIM_TAG.match(claim)
    lane = reviewer_of(prefix.group(1) if prefix else claim.split(":", 1)[0])
    phrases, identifiers = identity_tokens(claim)
    mine = phrases | identifiers
    if not mine:
        return set()
    number = severity[1]
    others = list(section.get(f"p{number}_open") or [])
    others += [item.get("claim", "") for item in section.get(f"p{number}_closed_claims") or [] if isinstance(item, dict)]
    others += [item.get("claim", "") for item in section.get(f"p{number}_restated_claims") or [] if isinstance(item, dict)]
    shared: set[str] = set()
    own_key = finding_key(claim, section_name, section) if section_name else finding_key(claim)
    for other in others:
        if other == claim:
            continue
        tag = CLAIM_TAG.match(other)
        other_lane = reviewer_of(tag.group(1) if tag else other.split(":", 1)[0])
        # A re-statement of the same finding (same finding key) is not
        # another finding; its vocabulary is this claim's own. Identity is
        # derived from the claims themselves — an equal key, or a
        # same-finding carry in either direction — never from the ledger's
        # `restates` link, which reopen_all pops before retire runs (Nabu P4
        # at 8966da2e: 48 claims read weak incrementally and strong on
        # regenerate). Every other claim of the lane counts (8f774d0c
        # round: an identifier-less neighbour had been read as the same
        # finding).
        other_key = finding_key(other, section_name, section) if section_name else finding_key(other)
        # Equal keys are one finding only when a carry links them: two
        # originals sharing a key (no backticked identifier to tell them
        # apart) still count toward each other's vocabulary, so a generic
        # head never retires either (Vulcan P4 at 8966da2e: 28 equal-key
        # groups had dropped from each other's shared set).
        if other_lane != lane or (
            other_key == own_key and (is_carry(other) or is_carry(claim))
        ):
            continue
        other_phrases, other_identifiers = identity_tokens(other)
        shared |= mine & (other_phrases | other_identifiers)
    return shared


def shares_its_kind(section: dict, severity: str, claim: str, section_name: str | None = None) -> bool:
    """Whether any kind phrase of the claim is shared within its lane and
    severity (the kind alone is then not an identity)."""
    phrases, _ = identity_tokens(claim)
    return bool(phrases & shared_vocabulary(section, severity, claim, section_name))


def open_lines_about(
    path: Path,
    claim: str,
    severity: str,
    section: dict | None = None,
    section_name: str | None = None,
) -> list[int]:
    """Lines of the transcript that record the claim's severity OPEN and speak
    about the claim: a transcript that records the claim OPEN cannot close it
    on another line (Vulcan P3 at 1a9f0aa). The OPEN item's head is bounded
    to the finding's own tag and anchor list, and the claim's shared
    vocabulary never relates (Ariadne P4 at 8966da2e: a carry parenthetical
    quoting another finding's kind had held a claim open)."""
    if not path.is_file():
        return []
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    found = []
    for index, line in enumerate(lines):
        if not is_open_line(line, severity):
            continue
        # The OPEN item's own head names the finding (its trailing prose may
        # mention other findings by name).
        if FINDING_LINE.match(line):
            head = line.split(" - ", 1)[0]
        else:
            opens = [m for m in STATUS_OPEN.finditer(line) if not _quoted(line, m.start(), m.end())]
            head = line[: opens[0].start()]
        head = " ".join(CATEGORY.findall(head) + PATH_TOKEN.findall(head))
        shared = shared_vocabulary(section, severity, claim, section_name) if section is not None else set()
        # A strong identity only (identifier, finding id, file:line anchor or
        # quoted phrase): one lane files several findings under one kind.
        if line_speaks_about(head, claim, strong=True, shared=shared, paths_ok=False):
            found.append(index + 1)
    return found


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


def claim_relation_problem(section: str, claim: str, path: Path, fields: dict | None = None) -> str | None:
    """The transcript must belong to the claim's section and lane, and to a
    scope strictly later than the claim's (c67b0729 round: the relation had
    been lexical only). A scope-tagged claim carries its scope; an untagged
    claim of a frozen `_revision_N` field takes the `on <sha>` scope of that
    field's `_note` (Vulcan P3 at 1a9f0aab: untagged claims had never been
    ancestry-checked)."""
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
    if not scope:
        scope = raising_scope(section, claim, fields)
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
    """`later` is a strict descendant of `earlier` (by short id, via git):
    equal scopes never count (Ariadne P4 at 8966da2e: `merge-base
    --is-ancestor x x` exits 0, so the equal-scope case read True)."""
    if not later_short or not earlier_short or later_short == earlier_short:
        return False
    key = (earlier_short, later_short)
    if key not in _scope_order:
        import subprocess

        done = subprocess.run(
            ["git", "merge-base", "--is-ancestor", earlier_short, later_short],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        _scope_order[key] = done.returncode == 0
    return _scope_order[key]


CATEGORY = re.compile(r"\[([^\]\n]+)\]")
PATH_TOKEN = re.compile(r"[A-Za-z0-9_./-]+\.(?:py|rs|md|json|toml)(?::[0-9,-]+)?")


STOP_WORDS = {"machineresearch", "evidence", "scripts", "review", "verdicts", "summary", "machine", "section",
              "closure", "closed", "record", "records", "findings", "finding", "carried", "still", "because",
              "should", "without", "through", "between", "against", "before", "after", "which", "their", "there"}


def content_words(text: str) -> set[str]:
    return {word for word in re.findall(r"[a-z][a-z0-9_]{5,}", text.lower()) if word not in STOP_WORDS}


_ledger_words: set[str] | None = None


def ledger_words() -> set[str]:
    """The ledger's own vocabulary: section names of the tracked summary and
    the per-package ledger fields (`p3_open`, `p4_closed_claims`, …)."""
    global _ledger_words
    if _ledger_words is None:
        words = {f"p{n}_{suffix}" for n in range(5) for suffix in ("open", "open_count", "closed_claims", "restated_claims")}
        words |= {"closed_claims", "restated_claims", "open_claims", "open_count", "machine_summary", "finding_register",
                  "claim_retirements", "verified_by", "original_note", "closure_note",
                  "package_open_claims", "package_closed_claims", "package_restated_claims",
                  "package_open", "package_open_count"}
        try:
            summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
            # Per-package section names (a section carries a status or a
            # per-package ledger), never ordinary summary fields.
            words |= {
                key for key, value in summary.items()
                if isinstance(value, dict) and ("status" in value or any(PACKAGE_OPEN_LIST.match(k) for k in value))
            }
        except (OSError, json.JSONDecodeError):
            pass
        _ledger_words = {word.lower() for word in words}
    return _ledger_words


def is_lane_field_name(word: str) -> bool:
    """A Council lane's field name or transcript stem (`nabu_architecture_review`,
    `vulcan_surface_review_revision_5`, `ariadne_implementation_review_closure`),
    a section name, a ledger field (`p3_open`, the `package_*` family) or a
    verdict-bearing field name
    (`final_vulcan_disposition`, `implementation_ariadne_review`) is the
    ledger's own vocabulary, never a finding identity (Ariadne P3 at
    6589c6ec; Ariadne P4 at 8966da2e: the verdict-bearing names had read
    as identities)."""
    if any(word.startswith(lane + "_") and "review" in word for lane in REVIEWERS):
        return True
    if word in ledger_words():
        return True
    return word in verdict_field_names()


_verdict_fields: set[str] | None = None


def verdict_field_names() -> set[str]:
    """Field names of the tracked summary that bear a lane verdict
    (`<lane>…review…`, `final_<lane>_disposition`, `implementation_…
    _review`, …): derived from the summary itself, never a fixed list."""
    global _verdict_fields
    if _verdict_fields is None:
        words: set[str] = set()
        try:
            summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
            for value in summary.values():
                if not isinstance(value, dict):
                    continue
                for key in value:
                    if any(lane in key for lane in REVIEWERS) and (
                        "review" in key or "disposition" in key or "delta" in key
                    ):
                        words.add(key)
        except (OSError, json.JSONDecodeError):
            pass
        _verdict_fields = words
    return _verdict_fields


LEDGER_PATHS = {
    "machineresearch/sley-2.0/machine-summary.json",
    "machine-summary.json",
    "evidence/review/finding-register.json",
    "finding-register.json",
    "evidence/review/claim-retirements.json",
    "claim-retirements.json",
    "evidence/release/ga-acceptance-report.json",
    "evidence/release/decision-dossier.json",
}


def closure_head(line: str) -> str:
    """The item text before its own CLOSED (or OPEN) status marker."""
    for pattern in (STATUS_CLOSED, STATUS_OPEN):
        for match in pattern.finditer(line):
            if _own_status(line, match):
                return line[: match.start()]
    if FINDING_LINE.match(line):
        return line.split(" - ", 1)[0]
    return line


def line_speaks_about(line: str, claim: str, strong: bool = False, shared: set[str] | None = None, paths_ok: bool = True) -> bool:
    """The closure line names the claim's finding identity (Nabu P2 at
    1a9f0aab: vocabulary overlap had let one line retire unrelated claims):

    - a specific kind phrase of the claim's bracketed tag, verbatim — a
      phrase of eight characters or more carrying a hyphen or a space
      (`fail-closed-gap`, `list-depth creep`, `contract-vs-mechanics`);
    - an identifier the claim names (a field, function or variable with an
      underscore, eight characters or more), a commit id, a finding id
      (`RW090-DEV-01`, `V-02`, `AR-06`, `S20-700-PACK-001`), a file anchor
      with its line numbers (`exchange.rs:2974-3001`), or a quoted phrase
      of the finding repeated verbatim;
    - or a path the claim names together with a word of its tag (a path
      alone is shared by every finding of a section; a generic one-word
      tag like `[record]` alone names nothing).

    With `strong` (a kind the lane shares, or the OPEN-line refusal) the
    line is read as its own head only and only an identifier, finding id,
    anchor span or quoted phrase standing in the head relates — the kind
    phrase, commit-id and path rules do not apply.
    """
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    if strong:
        # A strong identity must stand in the item's own head; evidence
        # prose after the status may name other findings (Vulcan P3 at
        # 79fdcc63).
        line = closure_head(line)
    lowered = line.lower()
    tags = [tag.lower() for tag in CATEGORY.findall(body[:120])]
    phrases = {
        phrase.strip()
        for tag in tags
        for phrase in re.split(r"[/—,;:]", tag)
        if phrase.strip() and phrase.strip() not in STOP_WORDS
    }
    shared = shared or set()
    specific = {
        phrase.replace("-", " ") for phrase in phrases
        if len(phrase) >= 8 and re.search(r"[- ]", phrase) and phrase not in shared
    }
    if not strong and any(phrase in lowered.replace("-", " ") for phrase in specific):
        return True
    path_text = " ".join(PATH_TOKEN.findall(body)).lower()
    # Identifiers are names the finding text carries, never the words of a
    # path it cites (a basename with underscores is a path, not a name).
    identifiers = {
        word for word in content_words(body)
        if "_" in word and len(word) >= 8 and word not in path_text and not is_lane_field_name(word) and word not in shared
    }
    if identifiers & {word for word in content_words(line) if not is_lane_field_name(word)}:
        return True
    # The ledger's own files are cited by most findings, so they never
    # relate (Ariadne P3 at 1a9f0aab: a line naming machine-summary.json
    # spoke about seven claims of one lane).
    paths = {token.split(":")[0] for token in PATH_TOKEN.findall(body)} - LEDGER_PATHS
    # A shared kind's own words are not tag words (Vulcan P3 at b58ac1e0:
    # under a shared kind, path + kind word let any later head of the lane
    # against one file close every finding of that kind).
    shared_words = {word for phrase in shared for word in re.findall(r"[a-z0-9]{5,}", phrase)}
    tag_words = {
        word for tag in tags for word in re.findall(r"[a-z0-9]{5,}", tag)
        if word not in STOP_WORDS and word not in shared_words
    }
    # A commit id relates only together with a tag word or a path (Vulcan
    # P3 at 1a9f0aab: a shared commit id alone bound unrelated lines).
    commits = set(re.findall(r"(?<![0-9a-zA-Z])[0-9a-f]{7,40}(?![0-9a-zA-Z])", body))
    commits = {commit for commit in commits if re.search(r"[a-f]", commit) and re.search(r"[0-9]", commit)}
    if not strong and any(commit in line for commit in commits) and (
        any(path in line for path in paths) or any(word in lowered for word in tag_words)
    ):
        return True
    quoted = {phrase for phrase in re.findall(r'"([^"]{12,})"', body)}
    if any(phrase in line for phrase in quoted):
        return True
    # Finding ids from the claim's heading and first sentence only (an id
    # mentioned in passing later in the description is not its identity).
    finding_ids = claim_finding_ids(body) - shared
    if any(re.search(rf"(?<![A-Z0-9-]){re.escape(fid)}(?![0-9])", line) for fid in finding_ids):
        return True
    anchors = {
        token.rsplit("/", 1)[-1]
        for token in PATH_TOKEN.findall(body)
        if ":" in token and token.split(":")[0] not in LEDGER_PATHS
    }
    if any(anchor in line for anchor in anchors):
        return True
    # The anchor's line span alone (`413-417,424-425`, `2974-3001`) when
    # the closure line cites it without the file name — a range or list,
    # never a single line number — and only when the line also names the
    # claim's file basename, or the claim cites no file at all (Vulcan P4
    # at b58ac1e0: a bare span had related across files).
    ranges = {
        token.split(":", 1)[1] for token in PATH_TOKEN.findall(body)
        if ":" in token and token.split(":")[0] not in LEDGER_PATHS and re.search(r"[-,]", token.split(":", 1)[1])
    }
    anchors_named = {
        token.split(":")[0].rsplit("/", 1)[-1] for token in PATH_TOKEN.findall(body)
        if token.split(":")[0] not in LEDGER_PATHS
    }
    if ranges and (not anchors_named or any(base in line for base in anchors_named)):
        if any(re.search(rf"(?<![0-9-]){re.escape(span)}(?![0-9-])", line) for span in ranges):
            return True
    # A path together with a tag word relates only under an unshared kind
    # (Nabu/Vulcan P2 at b58ac1e0: under a shared kind one generic head
    # `[<kind>] <path> — CLOSED` would close every finding of the lane
    # against that file; the strong read and the OPEN refusal are
    # symmetric — identifier, finding id, anchor or quoted phrase only).
    return (not strong) and paths_ok and any(path in line for path in paths) and any(word in lowered for word in tag_words)


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
    return resolved if resolved.is_file() and is_tracked(path) else None


_tracked: set[str] | None = None


def is_tracked(path: str) -> bool:
    """The transcript is in the git index (tracked or staged): an untracked
    file in a worktree is not a filed transcript (Vulcan P3 at 1a9f0aa).
    A failed git listing refuses everything (Nabu P4 at 6589c6ec);
    without git every path counts as tracked."""
    global _tracked
    if _tracked is None:
        import subprocess

        try:
            done = subprocess.run(["git", "ls-files", "-z", "--", "evidence/review", "machineresearch/sley-2.0/reviews"],
                                  cwd=ROOT, capture_output=True, text=True, check=False)
        except OSError:
            return True
        _tracked = set(done.stdout.split("\0")) if done.returncode == 0 else set()
    return path in _tracked


RETIREMENTS = ROOT / "evidence/review/claim-retirements.json"


def exact_claim_binding(section: str, claim: str, transcript: Path, lines: list[int]) -> bool:
    """The tracked retirement file names this exact claim, transcript and
    lines under an entry's `claims` list."""
    try:
        entries = json.loads(RETIREMENTS.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    relative = transcript.resolve().relative_to(ROOT.resolve()).as_posix()
    for entry in entries:
        if not isinstance(entry, dict) or entry.get("section") != section or entry.get("verified_by") != relative:
            continue
        for item in entry.get("claims") or []:
            if isinstance(item, dict) and item.get("claim") == claim and sorted(item.get("lines") or []) == sorted(lines):
                return True
    return False


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
                    or set(entry) not in ({"claim", "verified_by"}, {"claim", "verified_by", "binding"})
                    or not isinstance(entry["claim"], str)
                    or not isinstance(entry["verified_by"], str)
                    or entry.get("binding", "exact-claim") != "exact-claim"
                ):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field} entries are {{claim, verified_by[, binding: exact-claim]}} strings",
                    )
                raised = raising_severity(section, entry["claim"])
                if raised and raised != "P" + field[1]:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: the raising transcript records {raised}: {entry['claim'][:60]!r}",
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
                relation = claim_relation_problem(section, entry["claim"], resolved, value)
                if relation:
                    raise RegisterError(RegisterErrorCode.SUMMARY_INVALID, f"{section}.{field}: {relation}")
                text = resolved.read_text(encoding="utf-8", errors="replace").splitlines()
                if entry.get("binding") == "exact-claim":
                    # An exact-claim binding names the whole claim string and
                    # its lines in the tracked retirement file: the reviewer's
                    # own per-finding closure line, bound by hand where the
                    # finding-identity relation cannot see it (Nabu P2 at
                    # 1a9f0aab). It is refused unless the tracked file
                    # carries exactly that binding.
                    if not exact_claim_binding(section, entry["claim"], resolved, wanted):
                        raise RegisterError(
                            RegisterErrorCode.SUMMARY_INVALID,
                            f"{section}.{field}: exact-claim binding not in {RETIREMENTS.relative_to(ROOT)}: {entry['claim'][:60]!r}",
                        )
                elif not any(
                    line_speaks_about(
                        text[n - 1], entry["claim"],
                        strong=shares_its_kind(value, severity, entry["claim"], section),
                        shared=shared_vocabulary(value, severity, entry["claim"], section),
                    )
                    for n in wanted if 0 < n <= len(text)
                ):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: no cited line of {resolved.name} names the claim's finding identity",
                    )
                # A transcript that records the claim's severity OPEN cannot
                # close it on another line — exact-claim bindings included.
                if open_lines_about(resolved, entry["claim"], severity, value, section):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: {resolved.name} also records the claim OPEN at {open_lines_about(resolved, entry['claim'], severity, value, section)}",
                    )
            claims[f"{section}.{field}"] = len(item)
    return dict(sorted(claims.items()))


ANCHOR = re.compile(r"[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+")
IDENTIFIER = re.compile(r"`([A-Za-z_][A-Za-z0-9_.]*)")


def _own_identifier(text: str) -> str:
    """The finding's own backticked identifier: never a path's stem. A
    backticked word that prefixes a path token of the same text (or ends
    it as one) names the file, not the finding (Nabu P4 at b58ac1e0: the
    `retire_review_claims` stem had keyed an open claim like the retired
    guard)."""
    stems = set()
    for token in PATH_TOKEN.findall(text):
        base = token.split(":")[0].rsplit("/", 1)[-1]
        stems.add(base.lower())
        stems.add(re.sub(r"\.(?:py|rs|md|json|toml)$", "", base.lower()))
    for found in IDENTIFIER.finditer(text):
        word = found.group(1)
        if is_lane_field_name(word):
            continue
        if word.lower() in stems or any(word.lower() in stem or stem.startswith(word.lower()) for stem in stems if stem):
            continue
        return word
    return ""
# A leading carry clause names its carried root: `(carried from <sha>, …)`,
# `(prior from <sha>, …)` — a bare `(prior)`, `(carried, unchanged)` or any
# other prose parenthetical never makes a re-statement (Ariadne/Nabu/Vulcan
# P2 at 8966da2e: sha-less leading clauses had keyed five open
# `[predicate-precision]` findings without an identifier of their own).
CARRIED_FROM = re.compile(r"\bcarr(?:ied|y)\s+(?:over\s+)?from\s+([0-9a-f]{7,40})\b", re.I)
LEADING_CARRY = re.compile(r"^\((?:carried|carry|prior|residual)[^)]*from\s+[0-9a-f]{7,40}[^)]*\)\s*", re.I)


def finding_key(claim: str, section: str | None = None, fields: dict | None = None) -> tuple[str | None, str, str, str]:
    """(lane, kind, origin, anchor) — the identity of a finding across the
    rounds that re-state it (Ariadne P3 at b58ac1e0: a carried
    re-statement keys on the finding it carries): the lane, the bracketed
    kind, the round the finding originates from — the `carried from <sha>`
    scope the re-statement names, else the claim's raising scope — and the
    first path-like token outside the ledger's own files (line numbers
    stripped); a claim with no known origin keys on its description."""
    tag = CLAIM_TAG.match(claim)
    prefix = tag.group(1) if tag else claim.split(":", 1)[0]
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    kind = re.match(r"\[([^\]]+)\]", body)
    rest = body[kind.end():].strip() if kind else body
    # A re-statement is a claim that names its carried root: a leading
    # `(carried from <sha>, …)` clause or a `carried from <sha>` phrase — a
    # prose word (`prior`, `unchanged`, `again`) never makes one (Ariadne/
    # Nabu/Vulcan P2 at 8f774d0c). Either way the claim keys on its OWN
    # backticked identifier (or "" when it has none): a named re-statement
    # is the same finding as the claim at the named scope carrying that
    # identifier — never a wildcard over the lane/kind/file (Ariadne/Nabu/
    # Vulcan P2 at 8966da2e: `identifier = None` had matched every
    # identifier, folding five `[predicate-precision]` re-statements into
    # an unrelated closed root).
    rest = LEADING_CARRY.sub("", rest)
    path = ANCHOR.search(rest)
    anchor = path.group(0).split(":")[0] if path else ""
    if anchor in LEDGER_PATHS or anchor.rsplit("/", 1)[-1] in LEDGER_PATHS:
        anchor = ""
    if anchor:
        identifier = _own_identifier(rest)
        return (reviewer_of(prefix), kind.group(1).strip().lower() if kind else "", anchor, identifier)
    # A finding against the ledger itself: its kind and the round it
    # originates from (the `carried from <sha>` scope a re-statement names,
    # else the claim's raising scope), or its description when no round is
    # known.
    sha = CARRIED_FROM.search(body)
    origin = sha.group(1)[:7] if sha else (raising_scope(section, claim, fields) if section else (tag.group(2)[:7] if tag and tag.group(2) else None))
    if origin is None:
        description = rest.split(" - ", 1)[1] if " - " in rest else rest
        origin = "desc:" + re.sub(r"\s+", " ", description)[:40]
    ident = _own_identifier(rest)
    identifier = ident if ident else ""
    return (reviewer_of(prefix), kind.group(1).strip().lower() if kind else "", origin, identifier)


def same_finding(one: tuple, other: tuple) -> bool:
    """Two finding keys name one finding: same lane, kind and anchor/origin,
    and equal     identifiers (8f774d0c round: an absent identifier had matched
    any; 8966da2e round: a named re-statement's `None` had matched every
    identifier of its lane/kind/file). A named re-statement therefore meets
    only the claim carrying its own identifier — the fold additionally
    requires that claim to sit at the named scope."""
    return one == other


def is_carry(claim: str) -> bool:
    """The claim names its carried root: a leading `(carried from <sha>, …)`
    clause or a `carried from <sha>` phrase (8f774d0c round: a prose word
    such as `prior` or `unchanged` had made an original a re-statement;
    8966da2e round: a sha-less leading clause such as `(prior)` or
    `(carried, unchanged)` had done the same)."""
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    kind = re.match(r"\[([^\]]+)\]", body)
    rest = body[kind.end():].strip() if kind else body
    return CARRIED_FROM.search(body) is not None or LEADING_CARRY.match(rest) is not None


def named_carry_sha(claim: str) -> str | None:
    """The seven-hex root scope a re-statement names (`carried from <sha>`,
    whether as a phrase or inside a sha-bearing leading clause), else None.
    A fold resolves its root by this sha — never by list position or by the
    earliest scope alone (Ariadne/Nabu/Vulcan P2 at 8966da2e: five open
    `[predicate-precision]` findings naming 79fdcc6/b58ac1e/8f774d0 had
    folded into a closed root at 76ae15a)."""
    found = CARRIED_FROM.search(claim)
    if found:
        return found.group(1)[:7]
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    kind = re.match(r"\[([^\]]+)\]", body)
    rest = body[kind.end():].strip() if kind else body
    leading = LEADING_CARRY.match(rest)
    if leading:
        inner = re.search(r"from\s+([0-9a-f]{7,40})", leading.group(0), re.I)
        if inner:
            return inner.group(1)[:7]
    return None


def claim_scope(claim: str) -> str | None:
    tag = CLAIM_TAG.match(claim)
    return tag.group(2) if tag else None


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
            item_entries = item
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
                raised = raising_severity(section, entry["claim"])
                if raised and raised != "P" + severity:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: the raising transcript records {raised}: {entry['claim'][:60]!r}",
                    )
                # A tagged claim names its raising round: the raising
                # transcript must record it, or the binding is refused
                # (Vulcan P4 at 8966da2e: `None` had been accepted on no
                # match; untagged claims still resolve through transcripts
                # and notes).
                tag = CLAIM_TAG.match(entry["claim"])
                if tag and tag.group(2) and raised is None:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: no raising transcript records the tagged claim: {entry['claim'][:60]!r}",
                    )
                if entry["claim"] in open_list or entry["claim"] in closed or entry["restates"] == entry["claim"]:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: re-stated claim is also listed elsewhere: {entry['claim'][:60]!r}",
                    )
                # The fold's identity rule is the builder's, not the
                # generator's (Vulcan P3 at 1a9f0aab): same lane, kind,
                # anchor and identifier, a carried marker on the re-statement,
                # a strictly later scope than the claim it restates, and a
                # chain that ends at an open or retired claim (no cycle, no
                # dangling link).
                if (
                    not same_finding(finding_key(entry["claim"], section, value), finding_key(entry["restates"], section, value))
                    or finding_key(entry["claim"], section, value)[0] is None
                ):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: re-statement is not the same finding: {entry['claim'][:60]!r}",
                    )
                # The fold resolves its root by the named sha (Vulcan P2 at
                # 8966da2e); the ledger replays the same rule, except for an
                # exact-key inheritance into a retired claim (one finding,
                # one status — Ariadne P2 at 8966da2e).
                named_sha = named_carry_sha(entry["claim"])
                target_scope = claim_scope(entry["restates"]) or raising_scope(section, entry["restates"], value)
                retired_targets = {item.get("claim") for item in value.get(f"p{severity}_closed_claims") or [] if isinstance(item, dict)}
                exact_inherit = (
                    finding_key(entry["claim"], section, value) == finding_key(entry["restates"], section, value)
                    and entry["restates"] in retired_targets
                )
                if named_sha and not exact_inherit and (target_scope or "")[:7] != named_sha:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: re-statement names {named_sha} but restates a claim at {target_scope}: {entry['claim'][:60]!r}",
                    )
                # An exact-key inheritance into a retired claim needs
                # neither a carry phrase nor a later scope: it is the same
                # finding, already judged (Ariadne P2 at 8966da2e).
                if not exact_inherit:
                    if not is_carry(entry["claim"]):
                        raise RegisterError(
                            RegisterErrorCode.SUMMARY_INVALID,
                            f"{section}.{field}: re-statement carries no carried/prior marker: {entry['claim'][:60]!r}",
                        )
                    later, first = claim_scope(entry["claim"]), claim_scope(entry["restates"])
                    if later is None or (first is not None and not strictly_later_scope(later, first)):
                        raise RegisterError(
                            RegisterErrorCode.SUMMARY_INVALID,
                            f"{section}.{field}: re-statement is not a strictly later round: {entry['claim'][:60]!r}",
                        )
                links = {item["claim"]: item["restates"] for item in item_entries if isinstance(item, dict) and "claim" in item and "restates" in item}
                seen: set[str] = set()
                target = entry["restates"]
                while target in links:
                    if target in seen:
                        raise RegisterError(RegisterErrorCode.SUMMARY_INVALID, f"{section}.{field}: re-statement cycle at {target[:60]!r}")
                    seen.add(target)
                    target = links[target]
                if target not in open_list and target not in closed:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: restates a claim the section does not carry: {target[:60]!r}",
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

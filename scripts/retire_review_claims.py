#!/usr/bin/env python3
"""Retire per-package open claims a later review verified closed.

Finding register contract section 3 (revision 6). An entry of a section's
`pN_open` list is `"<field>[@<scope7>]: <finding>"` — the lane field that
raised it and, from revision 6, the seven-hex scope it was recorded at. It
is retired to `pN_closed_claims` as `{claim, verified_by}` when either

- automatic rule: a verdict field of the SAME lane in the section carries
  `PRIOR_..._PN_..._CLOSED` (the lane re-verified its carried findings item
  by item) and was recorded at a scope that is a strict descendant of the
  entry's scope; `verified_by` is that verdict's transcript
  (`<section>/<lane>*-<scope7>.md`, resolved by scope; closers are read
  from the live verdict fields and from every filed transcript's VERDICT
  line) with the cited closure lines that speak about the claim; an entry
  without a scope tag is never retired automatically; or
- an explicit retirement in `evidence/review/claim-retirements.json`
  naming `section`, `severities`, `prefixes` and the `verified_by`
  transcript path (with a reason), for entries the automatic rule cannot
  see (untagged entries, lane-less rows, closure reviews whose verdict token
  omitted the `PRIOR` suffix but whose transcript records the closure).

Nothing vanishes: a retired entry stays in the section under
`pN_closed_claims`, and `build_finding_register.py` refuses a retirement
whose transcript does not exist. Usage: retire_review_claims.py [--check]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from build_finding_register import closed_severities, cited_closure_lines, line_speaks_about  # noqa: E402

SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
RETIREMENTS = ROOT / "evidence/review/claim-retirements.json"
REVIEWERS = ("ariadne", "nabu", "vulcan", "merlin", "codex")
PRIOR = re.compile(r"_PRIOR_((?:P[0-4]_)*P[0-4])_CLOSED")
SCOPE = re.compile(r"\bon ([0-9a-f]{40})\b")
TRANSCRIPT = re.compile(r"transcript ((?:evidence/review/verdicts|machineresearch/sley-2\.0/reviews)/\S+?\.(?:md|log))")
TAG = re.compile(r"^([A-Za-z0-9_.]+?)(?:@([0-9a-f]{7,40}))?: ")


def role(field: str) -> str | None:
    """The lane a field, claim prefix or transcript name belongs to; dotted
    fields (`current_delta_review.<lane>`) split too (Ariadne P4 at 76ae15ab)."""
    for token in re.split(r"[._-]", field):
        if token in REVIEWERS:
            return token
    return None


def note_of(section: dict, field: str) -> str | None:
    if "." in field:
        parent, lane = field.split(".", 1)
        live = section.get(parent)
        text = live.get(lane + "_note") if isinstance(live, dict) else None
    else:
        text = section.get(field + "_note")
    return text if isinstance(text, str) else None


_ancestry: dict[tuple[str, str], bool] = {}


def strictly_later(later: str | None, earlier: str | None) -> bool:
    """`later` is a strict descendant of `earlier` in this repository."""
    if not later or not earlier or later.startswith(earlier) or earlier.startswith(later):
        return False
    key = (earlier, later)
    if key not in _ancestry:
        _ancestry[key] = (
            subprocess.run(
                ["git", "merge-base", "--is-ancestor", earlier, later],
                cwd=ROOT, check=False, capture_output=True,
            ).returncode
            == 0
        )
    return _ancestry[key]


def transcript_for(section_name: str, lane: str, scope: str | None, fallback: str | None) -> str | None:
    """The lane's transcript at `scope` (`<section>/<lane>*-<scope7>.md`), else the note's."""
    if scope:
        matches = sorted((ROOT / "evidence/review/verdicts" / section_name).glob(f"{lane}*-{scope[:7]}.md"))
        if len(matches) == 1:
            return matches[0].relative_to(ROOT).as_posix()
        # Zero or ambiguous matches never fall through to the note's path
        # silently (Vulcan P4 at 76ae15ab): a closer whose transcript cannot
        # be resolved is refused unless the note's path is one of the matches.
        if fallback and (ROOT / fallback) in matches:
            return fallback
        raise SystemExit(
            f"{section_name}: {len(matches)} transcripts match {lane}*-{scope[:7]}.md (note names {fallback})"
        )
    return fallback


VERDICT_LINE = re.compile(r"^VERDICT:\s*([A-Z0-9_]+)\s*$", re.M)
_resolved: dict[str, str | None] = {}


def resolve_scope(short: str) -> str | None:
    """The full commit id a transcript's scope stamp names (None when unknown)."""
    if short not in _resolved:
        done = subprocess.run(["git", "rev-parse", "--verify", "--quiet", f"{short}^{{commit}}"],
                              cwd=ROOT, capture_output=True, text=True, check=False)
        _resolved[short] = done.stdout.strip() if done.returncode == 0 and done.stdout.strip() else None
    return _resolved[short]


def transcript_closers(section_name: str) -> list[tuple[str, str, set[str], str | None, str | None]]:
    """Every filed transcript of the section whose VERDICT carries a PRIOR
    clause: (file stem, lane, closed severities, scope, path). Transcripts
    are the durable record, so the closers do not depend on which round's
    verdict currently occupies the live field (76ae15ab round: reading live
    fields only made the regeneration non-monotone)."""
    found = []
    directory = ROOT / "evidence/review/verdicts" / section_name
    if not directory.is_dir():
        return found
    for path in sorted(directory.glob("*.md")):
        stamp = re.search(r"-([0-9a-f]{7,40})\.md$", path.name)
        lane = role(path.name)
        if not stamp or lane is None:
            continue
        verdicts = VERDICT_LINE.findall(path.read_text(encoding="utf-8", errors="replace"))
        if not verdicts or not PRIOR.search(verdicts[-1]):
            continue
        severities = closed_severities(verdicts[-1])
        found.append((path.stem, lane, severities, resolve_scope(stamp.group(1)), path.relative_to(ROOT).as_posix()))
    return found


def closers(section: dict, section_name: str = "") -> list[tuple[str, str, set[str], str | None, str | None]]:
    """(field, lane, closed severities, scope, transcript) per PRIOR verdict."""
    found = []
    candidates: list[tuple[str, object]] = [
        (field, value) for field, value in section.items() if not field.endswith("_note")
    ]
    live = section.get("current_delta_review")
    if isinstance(live, dict):
        candidates.extend((f"current_delta_review.{lane}", live.get(lane)) for lane in REVIEWERS)
    for field, value in candidates:
        if not isinstance(value, str) or role(field) is None:
            continue
        if not PRIOR.search(value):
            continue
        # One grammar (Nabu P3 at 76227765): the closed set is the register's
        # `closed_severities`, never a second parse of the token.
        severities = closed_severities(value)
        note = note_of(section, field) or ""
        scope = SCOPE.search(note)
        transcript = TRANSCRIPT.search(note)
        scope_sha = scope.group(1) if scope else None
        found.append((
            field,
            role(field),
            severities,
            scope_sha,
            transcript_for(section_name, role(field), scope_sha, transcript.group(1) if transcript else None),
        ))
    seen = {item[4] for item in found if item[4]}
    found.extend(item for item in transcript_closers(section_name) if item[4] not in seen)
    return found


def speaking_lines(path: Path, severity: str, claim: str) -> list[int]:
    """Closure lines of `severity` that name the claim's category or path."""
    if not path.is_file():
        return []
    text = path.read_text(encoding="utf-8", errors="replace").splitlines()
    return [n for n in cited_closure_lines(path, severity) if line_speaks_about(text[n - 1], claim)]


def retire(summary: dict, retirements: list[dict]) -> int:
    retired = 0
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        lane_closers = closers(section, name)
        for severity in range(5):
            key = f"p{severity}_open"
            entries = section.get(key)
            if not isinstance(entries, list) or not entries:
                continue
            keep: list[str] = []
            closed = list(section.get(f"p{severity}_closed_claims", []))
            for entry in entries:
                verified = None
                tag = TAG.match(entry)
                prefix = tag.group(1) if tag else entry.split(":", 1)[0]
                scope = tag.group(2) if tag else None
                for field, lane, severities, closer_scope, transcript in lane_closers:
                    if (
                        lane == role(prefix)
                        and f"P{severity}" in severities
                        and transcript
                        and strictly_later(closer_scope, scope)
                    ):
                        lines = speaking_lines(ROOT / transcript, f"P{severity}", entry)
                        if not lines:
                            continue
                        verified = f"{transcript}#L{','.join(map(str, lines))} — {field} at {closer_scope[:8]} verified the carried P{severity} closed"
                        break
                for item in retirements:
                    if (
                        item["section"] == name
                        and severity in item["severities"]
                        and entry.startswith(tuple(item["prefixes"]))
                    ):
                        # A prefix names at least a whole lane field
                        # (`<field>[@scope]: `); a bare-lane or empty prefix
                        # would sweep the section (Vulcan P4 at 76ae15ab). Each
                        # matched claim is still bound to its own speaking line.
                        for prefix in item["prefixes"]:
                            head = re.match(r"^([A-Za-z0-9_.]+)(?:@[0-9a-f]{7,40})?:", prefix)
                            if not head or head.group(1) in REVIEWERS or role(head.group(1)) is None:
                                raise SystemExit(f"retirement for {name}: prefix too broad: {prefix!r}")
                        recorded = speaking_lines(ROOT / item["verified_by"], f"P{severity}", entry)
                        closure = cited_closure_lines(ROOT / item["verified_by"], f"P{severity}")
                        lines = item.get("lines") or recorded
                        # Every cited line records a closure of the severity and
                        # at least one speaks about the claim (the builder's rule).
                        if not lines or not set(lines) <= set(closure) or not set(lines) & set(recorded):
                            raise SystemExit(
                                f"retirement for {name} P{severity} cites {item['verified_by']}#L{lines} "
                                f"which records a closure of P{severity} at {closure}, naming the claim only at {recorded} :: {entry[:90]}"
                            )
                        verified = f"{item['verified_by']}#L{','.join(map(str, lines))} — {item['reason']}"
                if verified:
                    closed.append({"claim": entry, "verified_by": verified})
                    retired += 1
                else:
                    keep.append(entry)
            section[key] = keep
            section[f"{key}_count"] = len(keep)
            if closed:
                section[f"p{severity}_closed_claims"] = closed
    return retired


ANCHOR = re.compile(r"[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+")
IDENTIFIER = re.compile(r"`([A-Za-z_][A-Za-z0-9_.]*)")
CARRY = re.compile(r"\b(?:carried|carry|prior|residual|unchanged|still open|again|byte-identical)\b", re.I)


def finding_key(claim: str) -> tuple[str | None, str, str, str]:
    """(lane, category tag, anchor, identifier) — the identity of a finding
    across the rounds that re-state it: the lane that raised it, its
    bracketed kind, the first path-like token (line numbers stripped) or,
    without one, the first forty characters of its description, and the
    first backticked identifier (index suffix stripped) so that distinct
    findings citing one records file stay distinct."""
    tag = TAG.match(claim)
    prefix = tag.group(1) if tag else claim.split(":", 1)[0]
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    kind = re.match(r"\[([^\]]+)\]", body)
    rest = body[kind.end():].strip() if kind else body
    path = ANCHOR.search(rest)
    anchor = path.group(0).split(":")[0] if path else re.sub(r"\s+", " ", rest)[:40]
    ident = IDENTIFIER.search(rest)
    return (role(prefix), kind.group(1) if kind else "", anchor, ident.group(1) if ident else "")


def is_carry(claim: str) -> bool:
    """The claim's description marks itself as a carried/re-stated finding."""
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    return CARRY.search(body[:160]) is not None


def claim_scope(claim: str) -> str | None:
    tag = TAG.match(claim)
    return tag.group(2) if tag else None


def fold_restatements(summary: dict) -> int:
    """Fold a lane's later re-statement of a carried open finding into the
    earliest open claim for it (Nabu/Vulcan/Ariadne P4 at 76ae15ab: carried
    findings were listed once per round). The re-statement moves to
    `pN_restated_claims` as `{claim, restates}`; nothing vanishes, the
    earliest claim (whose scope a later closure must strictly postdate)
    stays open, and the closure line must still speak about it."""
    folded = 0
    for section in summary.values():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            entries = section.get(f"p{severity}_open")
            if not isinstance(entries, list) or len(entries) < 2:
                continue
            groups: dict[tuple, list[str]] = {}
            for entry in entries:
                groups.setdefault(finding_key(entry), []).append(entry)
            keep: list[str] = []
            restated = list(section.get(f"p{severity}_restated_claims", []))
            for entry in entries:
                group = groups[finding_key(entry)]
                if group[0] == entry or finding_key(entry)[0] is None or not is_carry(entry):
                    keep.append(entry)
                    continue
                earliest = group[0]
                later = claim_scope(entry)
                first = claim_scope(earliest)
                # Only a strictly later round's re-statement folds; an
                # untagged claim is the earliest by construction.
                if first is None and later is not None or (first and later and strictly_later(later, first)):
                    restated.append({"claim": entry, "restates": earliest})
                    folded += 1
                else:
                    keep.append(entry)
            section[f"p{severity}_open"] = keep
            section[f"p{severity}_open_count"] = len(keep)
            if restated:
                section[f"p{severity}_restated_claims"] = restated
    return folded


def reopen_all(summary: dict) -> None:
    """Return every closed claim to its open list (regeneration from scratch;
    the same transcripts must close it again, or it stays open)."""
    for section in summary.values():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            closed = section.pop(f"p{severity}_closed_claims", None)
            if not closed:
                continue
            entries = list(section.get(f"p{severity}_open") or [])
            for item in closed:
                if item["claim"] not in entries:
                    entries.append(item["claim"])
            section[f"p{severity}_open"] = entries
            section[f"p{severity}_open_count"] = len(entries)


def replay_problems(summary: dict) -> list[str]:
    """Closed claims whose cited lines no longer record a closure that speaks
    about them, or that are also listed open."""
    problems: list[str] = []
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        for severity in range(5):
            open_entries = set(section.get(f"p{severity}_open") or [])
            for item in section.get(f"p{severity}_closed_claims") or []:
                claim, verified_by = item.get("claim", ""), item.get("verified_by", "")
                path = verified_by.split("#")[0]
                cited = re.search(r"#L([0-9,]+)", verified_by)
                wanted = {int(n) for n in cited.group(1).split(",") if n} if cited else set()
                recorded = set(speaking_lines(ROOT / path, f"P{severity}", claim)) if path else set()
                closure = set(cited_closure_lines(ROOT / path, f"P{severity}")) if path else set()
                if not wanted or not wanted <= closure or not wanted & recorded:
                    problems.append(f"{name}.p{severity}_closed_claims: {verified_by[:100]} :: {claim[:60]}")
                if claim in open_entries:
                    problems.append(f"{name}.p{severity}: open and closed: {claim[:60]}")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if any claim is still retirable")
    parser.add_argument("--regenerate", action="store_true",
                        help="return every closed claim to its open list first, then retire from scratch")
    parser.add_argument("--fold-restatements", action="store_true",
                        help="fold later re-statements of a carried open finding into its earliest claim")
    args = parser.parse_args()
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    if args.regenerate:
        reopen_all(summary)
    folded = fold_restatements(summary) if args.fold_restatements else 0
    retirements = json.loads(RETIREMENTS.read_text(encoding="utf-8")) if RETIREMENTS.exists() else []
    retired = retire(summary, retirements)
    if args.check:
        # Replay (Vulcan/Nabu P4 at 76ae15ab): every recorded closed claim
        # must still verify against its transcript at the cited lines, and
        # no claim may be listed open and closed at once.
        stale = replay_problems(summary)
        result = "PASS" if retired == 0 and not stale else "FAIL"
        print(json.dumps({"retirable": retired, "stale_closures": stale, "result": result}, indent=2))
        return 0 if result == "PASS" else 1
    SUMMARY.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"retired": retired, "folded": folded, "result": "PASS"}))
    return 0


if __name__ == "__main__":
    sys.exit(main())

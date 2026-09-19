#!/usr/bin/env python3
"""Retire per-package open claims a later review verified closed.

Finding register contract section 3 (revision 6). An entry of a section's
`pN_open` list is `"<field>[@<scope7>]: <finding>"` — the lane field that
raised it and, from revision 6, the seven-hex scope it was recorded at. It
is retired to `pN_closed_claims` as `{claim, verified_by}` when either

- automatic rule: a verdict field of the SAME lane in the section carries
  `PRIOR_..._PN_..._CLOSED` (the lane re-verified its carried findings item
  by item) and was recorded at a scope that is a strict descendant of the
  entry's scope; `verified_by` is that verdict's transcript path (read from
  the field's `_note`, `transcript <path>`); an entry without a scope tag is
  never retired automatically; or
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
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
RETIREMENTS = ROOT / "evidence/review/claim-retirements.json"
REVIEWERS = ("ariadne", "nabu", "vulcan", "merlin", "codex")
PRIOR = re.compile(r"_PRIOR_((?:P[0-4]_)*P[0-4])_CLOSED")
SCOPE = re.compile(r"\bon ([0-9a-f]{40})\b")
TRANSCRIPT = re.compile(r"transcript ((?:evidence/review/verdicts|machineresearch/sley-2\.0/reviews)/\S+?\.(?:md|log))")
TAG = re.compile(r"^([A-Za-z0-9_.]+?)(?:@([0-9a-f]{7,40}))?: ")


def role(field: str) -> str | None:
    for token in field.split("_"):
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


def closers(section: dict) -> list[tuple[str, str, set[str], str | None, str | None]]:
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
        match = PRIOR.search(value)
        if not match:
            continue
        note = note_of(section, field) or ""
        scope = SCOPE.search(note)
        transcript = TRANSCRIPT.search(note)
        found.append((
            field,
            role(field),
            set(match.group(1).split("_")),
            scope.group(1) if scope else None,
            transcript.group(1) if transcript else None,
        ))
    return found


def retire(summary: dict, retirements: list[dict]) -> int:
    retired = 0
    for name, section in summary.items():
        if not isinstance(section, dict):
            continue
        lane_closers = closers(section)
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
                        verified = f"{transcript} — {field} at {closer_scope[:8]} verified the carried P{severity} closed"
                        break
                for item in retirements:
                    if (
                        item["section"] == name
                        and severity in item["severities"]
                        and entry.startswith(tuple(item["prefixes"]))
                    ):
                        verified = f"{item['verified_by']} — {item['reason']}"
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if any claim is still retirable")
    args = parser.parse_args()
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    retirements = json.loads(RETIREMENTS.read_text(encoding="utf-8")) if RETIREMENTS.exists() else []
    retired = retire(summary, retirements)
    if args.check:
        print(json.dumps({"retirable": retired, "result": "PASS" if retired == 0 else "FAIL"}))
        return 0 if retired == 0 else 1
    SUMMARY.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"retired": retired, "result": "PASS"}))
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Measure how far the M0 threat register's planned controls have been realized.

`docs/THREAT_REGISTER.md` is a planned-control map: it names, for each threat,
the failure code the implementation must produce, and it says explicitly that
its evidence paths are future outputs and must not be read as passing evidence.
Nothing measured the progress of that plan, so master goal section 26.6 ("all
P0/P1 threats have passing tests") had no tracked number.

This report locates each threat's expected failure symbol in the tree and
classifies it. Locating a symbol is not proof that the threat is mitigated: it
says the named control exists somewhere and, where a test or checker mentions
it, that something exercises it. The judgment stays with the independent
security review.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGISTER = ROOT / "docs/THREAT_REGISTER.md"
REPORT = ROOT / "evidence/security/threat-coverage-report.json"
CONTRACT = "sley2.threat-coverage-report.v1"
SEARCHED = {
    "implementation": ("crates",),
    "checkers": ("scripts",),
    "fuzz": ("fuzz/targets",),
    "oracle": ("oracle",),
}
TEST_MARKERS = ("#[test]", "def test_", "assert")
# This generator names example codes in its own prose, and the threat register
# is the source rather than an implementation, so neither may count as a
# located control.
SELF_REFERENCES = ("scripts/build_threat_coverage_report.py",)


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def register_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for line in REGISTER.read_text(encoding="utf-8").split("\n"):
        if not line.startswith("| T"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) < 7:
            continue
        rows.append(
            {
                "id": cells[0],
                "threat": cells[1],
                "severity": cells[2],
                "owner": cells[3],
                "expected_failure_code": cells[4].strip("`"),
                "required_test": cells[5],
                "planned_evidence_path": cells[6].strip("`"),
            }
        )
    return rows


def locate(symbol: str) -> dict[str, list[str]]:
    """Files that mention one expected failure symbol, by area."""
    found: dict[str, list[str]] = {}
    for area, trees in SEARCHED.items():
        hits: list[str] = []
        for tree in trees:
            base = ROOT / tree
            if not base.is_dir():
                continue
            for path in sorted(base.rglob("*")):
                if not path.is_file() or path.suffix not in (".rs", ".py"):
                    continue
                relative = str(path.relative_to(ROOT))
                if "/target/" in relative or "__pycache__" in relative:
                    continue
                if relative in SELF_REFERENCES:
                    continue
                text = path.read_text(encoding="utf-8", errors="ignore")
                if symbol in text:
                    hits.append(relative)
        if hits:
            found[area] = hits
    return found


def family_symbols() -> dict[str, set[str]]:
    """Every uppercase failure symbol the tree defines, grouped by family prefix.

    A threat's expected code may have been realized under a more specific name
    (the M0 register named `SCB_MALFORMED`; the codec ships `SCB_MAGIC_INVALID`
    and friends). Grouping by the first token lets the report show the review
    what the family actually contains instead of implying absence.
    """
    families: dict[str, set[str]] = {}
    for tree in ("crates", "scripts", "oracle"):
        base = ROOT / tree
        for path in sorted(base.rglob("*")):
            if not path.is_file() or path.suffix not in (".rs", ".py"):
                continue
            relative = str(path.relative_to(ROOT))
            if "/target/" in relative or "__pycache__" in relative or relative in SELF_REFERENCES:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            for symbol in re.findall(r'"([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)"', text):
                families.setdefault(symbol.split("_", 1)[0], set()).add(symbol)
    return families


def classify(found: dict[str, list[str]], evidence_present: bool) -> str:
    if evidence_present:
        return "PLANNED_EVIDENCE_PRESENT"
    if not found:
        return "SYMBOL_NOT_LOCATED"
    for paths in found.values():
        for relative in paths:
            text = (ROOT / relative).read_text(encoding="utf-8", errors="ignore")
            if any(marker in text for marker in TEST_MARKERS):
                return "SYMBOL_REALIZED_WITH_EXERCISE"
    return "SYMBOL_REALIZED_NO_EXERCISE_LOCATED"


def build_report() -> dict:
    rows = register_rows()
    families = family_symbols()
    threats = []
    for row in rows:
        found = locate(row["expected_failure_code"])
        evidence_present = (ROOT / row["planned_evidence_path"]).exists()
        threats.append(
            {
                **row,
                "state": classify(found, evidence_present),
                "located_in": {area: paths[:4] for area, paths in sorted(found.items())},
                "located_file_count": sum(len(paths) for paths in found.values()),
                "planned_evidence_present": evidence_present,
                "related_family_symbols": (
                    []
                    if found or evidence_present
                    else sorted(families.get(row["expected_failure_code"].split("_", 1)[0], set()))[:8]
                ),
            }
        )
    states: dict[str, int] = {}
    by_severity: dict[str, dict[str, int]] = {}
    for threat in threats:
        states[threat["state"]] = states.get(threat["state"], 0) + 1
        bucket = by_severity.setdefault(threat["severity"], {})
        bucket[threat["state"]] = bucket.get(threat["state"], 0) + 1
    blocking = [
        threat["id"]
        for threat in threats
        if threat["severity"] in ("P0", "P1") and threat["state"] == "SYMBOL_NOT_LOCATED"
    ]
    with_family = [
        threat["id"]
        for threat in threats
        if threat["state"] == "SYMBOL_NOT_LOCATED" and threat["related_family_symbols"]
    ]
    without_family = [
        threat["id"]
        for threat in threats
        if threat["state"] == "SYMBOL_NOT_LOCATED" and not threat["related_family_symbols"]
    ]
    report = {
        "contract": CONTRACT,
        "source": "docs/THREAT_REGISTER.md",
        "register_status": "M0 planned-control map; evidence paths are future outputs",
        "threat_count": len(threats),
        "states": states,
        "by_severity": by_severity,
        "p0_p1_without_located_symbol": blocking,
        "unlocated_with_realized_family": with_family,
        "unlocated_with_no_family_symbol": without_family,
        "threats": threats,
        "interpretation": (
            "locating a symbol proves the named control exists in the tree, not that the threat is "
            "mitigated; SYMBOL_REALIZED_WITH_EXERCISE means a file that mentions the symbol also "
            "carries a test or assertion, and PLANNED_EVIDENCE_PRESENT means the register's own "
            "evidence directory exists. SYMBOL_NOT_LOCATED is a work list for the independent "
            "security review, not a claim that the threat is untested: the M0 register named the "
            "failure code it expected, and a package may have realized the control under more "
            "specific codes. The judgment stays with the review."
        ),
        "independent_security_review": "PENDING",
    }
    report["report_digest"] = digest_of(report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    report = build_report()
    text = canonical(report)
    summary = {
        "threat_count": report["threat_count"],
        "states": report["states"],
        "p0_p1_without_located_symbol": len(report["p0_p1_without_located_symbol"]),
        "unlocated_with_realized_family": len(report["unlocated_with_realized_family"]),
        "unlocated_with_no_family_symbol": len(report["unlocated_with_no_family_symbol"]),
    }
    if args.check:
        current = REPORT.read_text(encoding="utf-8") if REPORT.exists() else None
        if current != text:
            print(
                canonical(
                    {
                        "mode": "check",
                        "result": "FAIL",
                        "detail": "the tracked threat coverage report differs from the derived report",
                    }
                ),
                end="",
            )
            return 1
        print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
        return 0
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(text, encoding="utf-8")
    print(canonical({"mode": "write", "result": "PASS", **summary}), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())

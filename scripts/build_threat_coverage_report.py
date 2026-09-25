#!/usr/bin/env python3
"""Measure how far the threat register's controls have been realized.

`docs/THREAT_REGISTER.md` is a control map: it names, for each threat, the
failure code the implementation must produce and the evidence that should
exercise it, and it says explicitly that a located control is not a claim of
mitigation while the independent security review is pending.
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

from rust_source_regions import rust_test_text


ROOT = Path(__file__).resolve().parents[1]
REGISTER = ROOT / "docs/THREAT_REGISTER.md"
REPORT = ROOT / "evidence/security/threat-coverage-report.json"
CONTRACT = "sley2.threat-coverage-report.v1"
SEARCHED = {
    "implementation": ("crates",),
    "checkers": ("scripts",),
    "fuzz": ("fuzz/targets",),
    "oracle": ("oracle",),
    # The benchmark, accounting, and release harnesses enforce controls too.
    "harness": ("bench",),
}
# A located test/corpus symbol is traceability, not proof of mitigation.
# Inline Rust test modules are bounded by syntax, including code after them.
EXERCISE_TREES = ("conformance", "fuzz/targets", "oracle")
EXERCISE_SUFFIXES = (".rs", ".py", ".json")
# This generator names example codes in its own prose, and the threat register
# is the source rather than an implementation, so neither may count as a
# located control.
SELF_REFERENCES = ("scripts/build_threat_coverage_report.py",)


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def structural_entries() -> set[str]:
    """Addendum entries whose realized control is structural, with no code symbol.

    T50 is the example: no crate references Git and the state root's binding
    excludes Git metadata, so there is nothing to search for. The absence is
    the control.
    """
    text = REGISTER.read_text(encoding="utf-8")
    if "## Realized codes" not in text:
        return set()
    section = text[text.index("## Realized codes") :]
    # A threat is structural only when every addendum row it carries names
    # no symbol: one symbol-bearing row makes it a located control (the
    # independent security review found T35 read as structural because its
    # first row described the admission type and its second named the code).
    with_symbol: set[str] = set()
    without_symbol: set[str] = set()
    for line in section.split("\n"):
        if not line.startswith("| T"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) < 3:
            continue
        if re.findall(
            r"`([A-Z][A-Z0-9_]+|[A-Za-z][A-Za-z0-9]*::[A-Za-z][A-Za-z0-9]*)`", cells[2]
        ):
            with_symbol.add(cells[0])
        else:
            without_symbol.add(cells[0])
    return without_symbol - with_symbol


def recorded_exercises() -> dict[str, list[str]]:
    """The addendum's "Exercised by" column, per threat.

    A test may reference a failure by enum variant rather than by its string,
    so a symbol search cannot see it. Where the register records the exercising
    test or corpus, that record is the evidence.
    """
    text = REGISTER.read_text(encoding="utf-8")
    if "## Realized codes" not in text:
        return {}
    section = text[text.index("## Realized codes") :]
    mapping: dict[str, list[str]] = {}
    for line in section.split("\n"):
        if not line.startswith("| T"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) >= 5 and cells[4]:
            # A threat may carry several addendum rows (T35: the query-owned
            # admission and the VM-owned cache key); every row's record counts.
            mapping.setdefault(cells[0], []).extend(
                re.findall(r"`([^`]+)`", cells[4]) or [cells[4]]
            )
    return mapping


def realized_codes() -> dict[str, list[str]]:
    """The register's addendum: threats whose shipped code differs from the plan.

    A realized control is not always a string constant. The reference adapter
    numbers its failures and names them as enum variants, so the addendum may
    record `AdapterErrorCode::PathInvalid`; that path is searched exactly like
    a code, because it is just as findable and just as specific.
    """
    text = REGISTER.read_text(encoding="utf-8")
    if "## Realized codes" not in text:
        return {}
    section = text[text.index("## Realized codes") :]
    mapping: dict[str, list[str]] = {}
    for line in section.split("\n"):
        if not line.startswith("| T"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) < 3:
            continue
        codes = re.findall(r"`([A-Z][A-Z0-9_]+)`", cells[2])
        codes += re.findall(r"`([A-Za-z][A-Za-z0-9]*::[A-Za-z][A-Za-z0-9]*)`", cells[2])
        if codes:
            merged = mapping.setdefault(cells[0], [])
            merged.extend(code for code in codes if code not in merged)
    return mapping


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
                if names_symbol(symbol, text):
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


def exercise_sources() -> list[tuple[str, str]]:
    """Every (relative path, exercising text) pair a symbol may be reached from.

    A Rust file contributes only complete cfg(test) inline modules, so a
    production match before or after a module never counts; a file under a
    `tests/` directory, a `test_*.py`, and every corpus, fuzz target, and
    oracle file contributes its whole text.
    """
    sources: list[tuple[str, str]] = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        relative = str(path.relative_to(ROOT))
        if "/target/" in relative:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        if "/tests/" in relative:
            sources.append((relative, text))
            continue
        tests = rust_test_text(text)
        if tests:
            sources.append((f"{relative}#tests", tests))
    for tree in ("scripts", "bench"):
        for path in sorted((ROOT / tree).rglob("*.py")):
            relative = str(path.relative_to(ROOT))
            if "__pycache__" in relative:
                continue
            if path.name.startswith("test_") or "/tests/" in relative:
                sources.append((relative, path.read_text(encoding="utf-8", errors="ignore")))
    for tree in EXERCISE_TREES:
        base = ROOT / tree
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            relative = str(path.relative_to(ROOT))
            if not path.is_file() or path.suffix not in EXERCISE_SUFFIXES:
                continue
            if "/target/" in relative or "__pycache__" in relative:
                continue
            sources.append((relative, path.read_text(encoding="utf-8", errors="ignore")))
    return sources


def names_symbol(symbol: str, text: str) -> bool:
    """Match a whole identifier/path, never a longer symbol containing it."""
    return re.search(rf"(?<![A-Za-z0-9_]){re.escape(symbol)}(?![A-Za-z0-9_])", text) is not None


def exercised_in(symbols: list[str], sources: list[tuple[str, str]]) -> list[str]:
    """The exercising sources that name any of the searched symbols."""
    return [relative for relative, text in sources if any(names_symbol(symbol, text) for symbol in symbols)]


def classify(found: dict[str, list[str]], evidence_present: bool, exercised: list[str]) -> str:
    if evidence_present:
        return "PLANNED_EVIDENCE_PRESENT"
    if not found:
        return "SYMBOL_NOT_LOCATED"
    if exercised:
        return "SYMBOL_REALIZED_WITH_EXERCISE"
    return "SYMBOL_REALIZED_NO_EXERCISE_LOCATED"


def build_report() -> dict:
    rows = register_rows()
    families = family_symbols()
    realized = realized_codes()
    structural = structural_entries()
    exercises = recorded_exercises()
    sources = exercise_sources()
    threats = []
    for row in rows:
        codes = realized.get(row["id"], [row["expected_failure_code"]])
        found: dict[str, list[str]] = {}
        for code in codes:
            for area, paths in locate(code).items():
                found[area] = sorted(set(found.get(area, [])) | set(paths))
        evidence_present = (ROOT / row["planned_evidence_path"]).exists()
        exercised = exercised_in(codes, sources)
        threats.append(
            {
                **row,
                "state": (
                    "STRUCTURAL_CONTROL_RECORDED"
                    if row["id"] in structural
                    else "SYMBOL_REALIZED_WITH_EXERCISE"
                    if found
                    and classify(found, evidence_present, exercised)
                    == "SYMBOL_REALIZED_NO_EXERCISE_LOCATED"
                    and row["id"] in exercises
                    else classify(found, evidence_present, exercised)
                ),
                "recorded_exercise": exercises.get(row["id"], []),
                "exercised_in": exercised[:6],
                "exercised_source_count": len(exercised),
                "searched_codes": codes,
                "realized_code_recorded": row["id"] in realized,
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
        "register_status": "control map for all 56 threats; coverage measured here; independent review pending",
        "threat_count": len(threats),
        "states": states,
        "by_severity": by_severity,
        "p0_p1_without_located_symbol": blocking,
        "unlocated_with_realized_family": with_family,
        "unlocated_with_no_family_symbol": without_family,
        "threats": threats,
        "interpretation": (
            "locating a symbol proves the named control exists in the tree, not that the threat is "
            "mitigated; SYMBOL_REALIZED_WITH_EXERCISE means a test region, test file, conformance "
            "corpus, fuzz target, or oracle names the symbol (or the addendum records the test that "
            "reaches it by variant), never that a production file merely carries some assertion; "
            "SYMBOL_REALIZED_NO_EXERCISE_LOCATED means the control exists but nothing found reaches "
            "it, and PLANNED_EVIDENCE_PRESENT means the register's own "
            "evidence directory exists. SYMBOL_NOT_LOCATED is a work list for the independent "
            "security review, not a claim that the threat is untested. STRUCTURAL_CONTROL_RECORDED "
            "means the addendum records a control that is the absence of something, so no symbol "
            "exists to find. The M0 register named the "
            "failure code it expected, and a package may have realized the control under more "
            "specific codes, which the register's Realized codes addendum records as they are "
            "traced. The judgment stays with the review."
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

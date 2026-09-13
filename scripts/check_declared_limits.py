#!/usr/bin/env python3
"""Verify every publicly declared limit value appears in a contract.

A decoder limit that decides validity is one of the nine epoch-frozen facts
(`EPOCH_MIGRATION_POLICY_V1.md` section 1), and the contracts state their
limits as exact numbers. Nothing checked that the crates' public `MAX_*`
constants still match, so a raised ceiling could change acceptance while every
document still stated the old bound.

Repair round 8 (invariant audit): the old test was "this number appears
somewhere in the concatenated spec tree", so raising a limit to any value
another contract already states passed. The test is now coupled: a value
must appear in a spec file that also names the constant. A bare number
anywhere else is not evidence for this constant. Constants whose name no
contract states keep the whole-tree value test as a fallback but are
reported as weak evidence, so a reviewer can see exactly how thin the
claim is. Non-literal initializers (`(1 << 53) - 1`, `u16::MAX`) are
evaluated where the meaning is platform-independent and otherwise
reported as unevaluated with the same name-coupling requirement.

Repair round 9 (invariant audit): the checker files tracked
`evidence/build/declared-limits.json` (write mode) and `--check` compares
against it, so the weak tier the summary records is drift-checked
instead of re-derived-and-forgotten. `make quick` runs `--check`.

The audit is deliberately narrow: it reads public constants only, because a
test-local bound is not a declared limit, and it checks that the value appears
somewhere in `docs/spec/`. It does not judge which contract owns which limit;
that stays with the owning package's checker.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONTRACT = "sley2.declared-limits.v1"
REPORT = ROOT / "evidence/build/declared-limits.json"
DECLARATION = re.compile(
    r"pub(?:\(crate\))? const (MAX_[A-Z0-9_]+):\s*\w+\s*=\s*([^;]+);"
)
NUMERIC = re.compile(r"[0-9][0-9_]*")
SHIFT = re.compile(r"\(\s*1\s*<<\s*(\d+)\s*\)\s*([+-])\s*(\d+)")
TYPE_MAX = re.compile(r"u(8|16|32|64)::MAX")
# Below this a bare number is too common in prose to carry evidence.
SIGNIFICANT = 16


def spec_texts() -> dict[str, str]:
    return {
        str(path): path.read_text(encoding="utf-8", errors="ignore")
        for path in sorted((ROOT / "docs/spec").rglob("*.md"))
    }


def spec_text() -> str:
    return "\n".join(spec_texts().values())


def spellings(value: int) -> set[str]:
    """The ways a contract may write one number."""
    plain = str(value)
    grouped = f"{value:,}"
    rust = plain
    parts: list[str] = []
    while len(rust) > 3:
        parts.insert(0, rust[-3:])
        rust = rust[:-3]
    parts.insert(0, rust)
    forms = {plain, grouped, "_".join(parts)}
    # Powers-of-two idiom: contracts state 2^53 - 1, not 9007199254740991.
    for candidate in (value - 1, value + 1):
        power = candidate.bit_length() - 1
        if candidate > 0 and (1 << power) == candidate:
            sign = "-" if candidate == value + 1 else "+"
            for template in ("2^{n} {s} 1", "2^{n}{s}1"):
                forms.add(template.format(n=power, s=sign))
    return forms


def evaluate(initializer: str) -> int | None:
    """A platform-independent value for simple initializers, else None."""
    text = initializer.strip()
    if NUMERIC.fullmatch(text):
        return int(text.replace("_", ""))
    shift = SHIFT.fullmatch(text)
    if shift:
        value = 1 << int(shift.group(1))
        return value + int(shift.group(3)) if shift.group(2) == "+" else value - int(shift.group(3))
    type_max = TYPE_MAX.fullmatch(text)
    if type_max:
        return (1 << int(type_max.group(1))) - 1
    return None


def check_constant(
    name: str, value: int | None, initializer: str, docs: dict[str, str]
) -> dict[str, object]:
    """The evidence grade for one declared limit.

    `documented`: a spec file names the constant and states the value.
    `weak`: the value appears somewhere but no file names the constant
    (the old whole-tree test; a shared value is not evidence for this
    constant). `undocumented`: the value appears nowhere, or (for an
    unevaluated initializer) the name appears nowhere.
    """
    named = [path for path, text in docs.items() if name in text]
    if value is None:
        grade = "documented" if named else "undocumented"
        return {"constant": name, "grade": grade, "initializer": initializer}
    forms = spellings(value)
    if any(any(form in docs[path] for form in forms) for path in named):
        return {"constant": name, "grade": "documented", "value": value}
    if value >= SIGNIFICANT and any(
        any(form in text for form in forms) for text in docs.values()
    ):
        return {"constant": name, "grade": "weak", "value": value}
    return {"constant": name, "grade": "undocumented", "value": value}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the derived report with the tracked one instead of writing it",
    )
    arguments = parser.parse_args()
    docs = spec_texts()
    declared = 0
    documented = 0
    undocumented = []
    weak = []
    unevaluated = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        for match in DECLARATION.finditer(path.read_text(encoding="utf-8", errors="ignore")):
            name, initializer = match.group(1), match.group(2)
            declared += 1
            value = evaluate(initializer)
            verdict = check_constant(name, value, initializer.strip(), docs)
            verdict["source"] = str(path.relative_to(ROOT))
            if verdict["grade"] == "undocumented":
                undocumented.append(verdict)
            elif verdict["grade"] == "weak":
                weak.append({k: verdict[k] for k in ("constant", "value", "source")})
            else:
                documented += 1
                if value is None:
                    unevaluated.append(
                        {k: verdict[k] for k in ("constant", "initializer", "source")}
                    )
    result = {
        "contract": CONTRACT,
        "declared_limits": declared,
        "documented_limits": documented,
        "weak_limits": len(weak),
        "undocumented_limits": len(undocumented),
        "scope": "PUBLIC CONSTANTS ONLY; OWNERSHIP STAYS WITH THE OWNING PACKAGE'S CHECKER",
        "undocumented": undocumented,
        "weak_evidence": weak,
        "unevaluated_initializers": unevaluated,
        "result": "PASS" if not undocumented else "FAIL",
    }
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if arguments.check:
        current = REPORT.read_text(encoding="utf-8") if REPORT.is_file() else None
        if current != text:
            print(
                json.dumps(
                    {
                        "mode": "check",
                        "result": "FAIL",
                        "detail": "the tracked limits report differs from the derived report",
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
            return 1
        print(
            json.dumps(
                {
                    "mode": "check",
                    "result": "PASS",
                    "declared_limits": declared,
                    "weak_limits": len(weak),
                    "undocumented_limits": len(undocumented),
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if not undocumented else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Verify every publicly declared limit value appears in a contract.

A decoder limit that decides validity is one of the nine epoch-frozen facts
(`EPOCH_MIGRATION_POLICY_V1.md` section 1), and the contracts state their
limits as exact numbers. Nothing checked that the crates' public `MAX_*`
constants still match, so a raised ceiling could change acceptance while every
document still stated the old bound.

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
DECLARATION = re.compile(
    r"pub(?:\(crate\))? const (MAX_[A-Z0-9_]+):\s*\w+\s*=\s*([0-9][0-9_]*)"
)
# Below this a bare number is too common in prose to carry evidence.
SIGNIFICANT = 16


def spec_text() -> str:
    return "\n".join(
        path.read_text(encoding="utf-8", errors="ignore")
        for path in sorted((ROOT / "docs/spec").rglob("*.md"))
    )


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
    return {plain, grouped, "_".join(parts)}


def main() -> int:
    argparse.ArgumentParser(description=__doc__).parse_args()
    docs = spec_text()
    declared = 0
    undocumented = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        for match in DECLARATION.finditer(path.read_text(encoding="utf-8", errors="ignore")):
            value = int(match.group(2).replace("_", ""))
            declared += 1
            if value >= SIGNIFICANT and not any(text in docs for text in spellings(value)):
                undocumented.append(
                    {
                        "constant": match.group(1),
                        "value": value,
                        "source": str(path.relative_to(ROOT)),
                    }
                )
    result = {
        "contract": CONTRACT,
        "declared_limits": declared,
        "scope": "PUBLIC CONSTANTS ONLY; OWNERSHIP STAYS WITH THE OWNING PACKAGE'S CHECKER",
        "undocumented": undocumented,
        "result": "PASS" if not undocumented else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not undocumented else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Check a golden directory against its SHA256SUMS file.

Every listed file must exist with the recorded SHA-256, the listing must name
every regular file under its directory, and no name may repeat. Any mismatch prints
each problem to stderr and exits 1. The default target is
crates/sley-tests/golden/SHA256SUMS, which nothing checked before.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMS = (ROOT / "crates/sley-tests/golden/SHA256SUMS",)
LINE = re.compile(r"^([0-9a-f]{64})  (\S+)$")


def sums_problems(sums: Path) -> list[str]:
    """Every disagreement between a SHA256SUMS file and its directory."""
    if not sums.is_file():
        return [f"missing:{sums.name}"]
    directory = sums.parent
    problems: list[str] = []
    listed: dict[str, str] = {}
    for number, line in enumerate(sums.read_text(encoding="utf-8").splitlines(), 1):
        match = LINE.match(line)
        if match is None:
            problems.append(f"malformed:line {number}")
            continue
        digest, name = match.groups()
        if name in listed:
            problems.append(f"duplicate:{name}")
        listed[name] = digest
    # Recursive, so a file hidden in a subdirectory is reported as unlisted.
    present = {path.relative_to(directory).as_posix() for path in directory.rglob("*") if path.is_file() and path != sums}
    for name in sorted(set(listed) - present):
        problems.append(f"missing:{name}")
    for name in sorted(present - set(listed)):
        problems.append(f"unlisted:{name}")
    for name in sorted(set(listed) & present):
        actual = hashlib.sha256((directory / name).read_bytes()).hexdigest()
        if actual != listed[name]:
            problems.append(f"mismatch:{name}: recorded {listed[name]}, actual {actual}")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sums", nargs="*", type=Path, help="SHA256SUMS files (default: the sley-tests golden set)")
    arguments = parser.parse_args(argv)
    targets = [path.resolve() for path in arguments.sums] or list(DEFAULT_SUMS)
    report = {}
    failed = False
    for sums in targets:
        problems = sums_problems(sums)
        label = sums.relative_to(ROOT).as_posix() if sums.is_relative_to(ROOT) else sums.name
        report[label] = problems
        for problem in problems:
            failed = True
            print(f"FAIL {label}: {problem}", file=sys.stderr)
    print(json.dumps({"problems": report, "result": "FAIL" if failed else "PASS"}, indent=2, sort_keys=True))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

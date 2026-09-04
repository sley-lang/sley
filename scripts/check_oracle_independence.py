#!/usr/bin/env python3
"""Fail if an independent oracle acquires a Rust implementation dependency.

Nineteen fixture families are checked by an independent oracle, and only seven
of those live under `oracle/scb1/src`: twelve are `scripts/check_*_vector.py`
files run through the same environment. Scanning only the package left twelve
oracles outside the check that vouches for them, so this audit reads every
oracle the S20-730 coverage map names, wherever it lives.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT / "oracle/scb1"
forbidden = (
    "subprocess",
    "cargo",
    "sley-id",
    "sley_id",
    "sley-mutate",
    "sley_mutate",
    "crates/",
    "value_generated.rs",
    "target/",
    # Linking the Rust library would be an implementation dependency that no
    # string above catches.
    "ctypes",
    "cffi",
    "os.system",
    "popen",
)
def coverage_oracles() -> list[Path]:
    """Every `scripts/` oracle the S20-730 coverage map runs."""
    import re

    builder = (ROOT / "scripts/build_independent_conformance_report.py").read_text(
        encoding="utf-8"
    )
    start = builder.index("COVERAGE: dict")
    section = builder[start : builder.index("\n}", start)]
    return [
        ROOT / "scripts" / name
        for name in sorted(set(re.findall(r"scripts/(check_[a-z0-9_]+\.py)", section)))
    ]


sources = sorted((ORACLE / "src").rglob("*.py")) + coverage_oracles()
problems: list[str] = []
for path in sources:
    if not path.is_file():
        problems.append(f"{path.relative_to(ROOT)} is named by the coverage map but absent")
        continue
    text = path.read_text(encoding="utf-8")
    for marker in forbidden:
        if marker in text:
            problems.append(
                f"{path.relative_to(ROOT)} contains forbidden marker {marker!r}"
            )
    # Dynamic loading is allowed only between oracles: reusing another
    # oracle's decoder keeps both independent of the Rust implementation,
    # while loading anything else would not.
    import re as _re

    for loaded in _re.findall(r'spec_from_file_location\([^,]+,\s*([^)]+)\)', text):
        if "scripts/" not in loaded and "oracle/" not in loaded:
            problems.append(
                f"{path.relative_to(ROOT)} loads {loaded.strip()} which is not another oracle"
            )

result = {
    "contract": "s20-130-oracle-independence-v1",
    "result": "FAIL" if problems else "PASS",
    "python_sources": len(sources),
    "package_sources": len(list((ORACLE / "src").rglob("*.py"))),
    "coverage_map_oracles": len(coverage_oracles()),
    "forbidden_markers": list(forbidden),
    "problems": problems,
}
print(json.dumps(result, indent=2, sort_keys=True))
if problems:
    raise SystemExit(1)

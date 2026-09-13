#!/usr/bin/env python3
"""Run the Rust lint gate and record a tracked lint report.

Invariant-audit repair round 8: `make lint` ran in no aggregate target,
produced no tracked artifact, and was named by no contract, so
`warnings_now: 0` had no evidence behind it. This script is the gate's
single source of truth: it runs `cargo fmt --check` and workspace clippy
with warnings denied, writes the tracked
`evidence/build/lint-report.json`, and exits nonzero on any failure.
`check-changed` depends on `lint`, so the gate runs in the broad
aggregate and its result is filed, not asserted.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "evidence/build/lint-report.json"


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(args), cwd=ROOT, check=False, capture_output=True, text=True
    )


def main() -> int:
    fmt = run("cargo", "fmt", "--all", "--check")
    clippy = run(
        "cargo",
        "clippy",
        "--no-deps",
        "--workspace",
        "--all-targets",
        "--locked",
        "--keep-going",
        "--message-format=short",
        "--",
        "-D",
        "warnings",
    )
    combined = clippy.stderr + "\n" + clippy.stdout
    generated = re.findall(r"generated (\d+) warning", combined)
    if generated:
        # Warnings not denied: cargo's own per-crate totals are exact.
        warnings = sum(int(count) for count in generated)
    else:
        # -D warnings renders lint diagnostics as file:line:col errors.
        warnings = sum(
            1
            for line in combined.splitlines()
            if re.match(r"\S+:\d+:\d+: (warning|error)", line)
        )
    head = run("git", "rev-parse", "HEAD").stdout.strip()
    result = {
        "contract": "sley2.lint-report.v1",
        "commit": head,
        "fmt_clean": fmt.returncode == 0,
        "fmt_detail": (fmt.stderr or fmt.stdout).strip()[:2000],
        "clippy_clean": clippy.returncode == 0,
        "clippy_warnings": warnings,
        "result": "PASS" if fmt.returncode == 0 and clippy.returncode == 0 else "FAIL",
    }
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["result"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())

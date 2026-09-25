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


LINT_INPUTS = ("crates/", ".cargo/", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
               "clippy.toml", "rustfmt.toml")


def dirty_worktree_paths(porcelain_z: str) -> list[str]:
    """Every dirty path of a `git status --porcelain -z` listing (sorted;
    renames contribute their destination)."""
    fields = porcelain_z.split("\0")
    paths: list[str] = []
    index = 0
    while index < len(fields):
        entry = fields[index]
        index += 1
        if not entry:
            continue
        status, path = entry[:2], entry[3:]
        paths.append(path)
        if "R" in status or "C" in status:
            index += 1  # the rename/copy source follows in its own field
    return sorted(set(paths))


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
    # The report names the commit it was recorded at; a dirty tree cannot
    # say which tree was linted, so the record carries the fact and the
    # packaging checker binds the commit to the candidate (Vulcan P4 at
    # 92fa6646).
    # `-z` keeps paths with spaces, quotes or non-ASCII bytes literal (v1
    # porcelain quotes them, so they never matched the prefix tuple — Vulcan
    # P4 at c04539b9..76ae15ab); a rename entry carries its source path in
    # the following NUL field and the destination is the path linted.
    dirty_paths = dirty_worktree_paths(run("git", "status", "--porcelain", "-z").stdout)
    dirty_inputs = sorted(path for path in dirty_paths if path.startswith(LINT_INPUTS))
    result = {
        "contract": "sley2.lint-report.v1",
        "commit": head,
        "working_tree_clean": not dirty_paths,
        "dirty_paths": dirty_paths,
        "lint_inputs_clean": not dirty_inputs,
        "dirty_lint_inputs": dirty_inputs,
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

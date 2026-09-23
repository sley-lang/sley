"""Source identity line for the deterministic witness logs.

A witness log names the commit its code ran at and, when tracked files
differ from that commit, the exact dirty paths. Witness log outputs
(`bench/live/succ-trials-*/`) are excluded from the dirty set: a
preceding witness rewriting its own tracked log is evidence output, not
code drift, and must not read as either.
"""

from __future__ import annotations

import subprocess
from pathlib import Path

WITNESS_OUTPUT_PREFIX = "bench/live/succ-trials-"


def source_identity(root: Path) -> str:
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root,
                          capture_output=True, text=True, check=False)
    status = subprocess.run(["git", "status", "--porcelain",
                             "--untracked-files=no"], cwd=root,
                            capture_output=True, text=True, check=False)
    commit = head.stdout.strip() if head.returncode == 0 else ""
    if not commit or status.returncode != 0:
        return "source unknown"
    dirty = sorted(
        path for path in (line[3:] for line in status.stdout.splitlines()
                          if len(line) > 3)
        if not path.startswith(WITNESS_OUTPUT_PREFIX))
    if dirty:
        return f"source {commit} (tracked changes: {', '.join(dirty)})"
    return f"source {commit} (clean apart from witness outputs)"

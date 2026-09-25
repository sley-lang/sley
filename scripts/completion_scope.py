#!/usr/bin/env python3
"""The contract text a completion review names, across the public-history cut.

The S20-300 and S20-620 checkers bind a lane's `<key>_revision_<N>` PASS
to the commit its note names and read that commit's copy of the contract.
A commit in the repository is read with git. A commit that exists only in
the archived history (see `history_ledger`) cannot be, so its contract's
Status line was frozen once at the cut in
`evidence/history/completion-reviewed-scopes.json` and is read from there.
An archived commit with no frozen entry, or a commit in neither the
repository nor the ledger, has no text: the caller fails closed.
"""

from __future__ import annotations

import functools
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import history_ledger  # noqa: E402  (sibling module)

RECORD = "evidence/history/completion-reviewed-scopes.json"


@functools.lru_cache(maxsize=None)
def _frozen(root: Path) -> dict[tuple[str, str], str]:
    path = root / RECORD
    if not path.exists():
        return {}
    entries = json.loads(path.read_text(encoding="utf-8")).get("entries", [])
    return {(entry["commit"], entry["path"]): entry["status_line"] for entry in entries}


def reviewed_text(commit: str, spec_path: str, root: Path) -> str | None:
    """`commit`'s copy of `spec_path` (live), its frozen Status line (archived), or None."""
    if history_ledger.is_archived(commit):
        return _frozen(root).get((history_ledger.resolve(commit), spec_path))
    shown = subprocess.run(
        ["git", "show", f"{commit}:{spec_path}"],
        cwd=root, capture_output=True, text=True, check=False,
    )
    return shown.stdout if shown.returncode == 0 else None

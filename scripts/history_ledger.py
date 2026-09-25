#!/usr/bin/env python3
"""Commit identity and ancestry across the public-history cut.

The public repository starts at a single root commit whose tree is the
sanitized export of the archived working history's tip. Records written
before that cut (review transcripts, attestations, proofs, anchors) name
commits that exist only in the archive, so ``git merge-base`` in a public
clone cannot see them.

``evidence/history/pre-public-history.json`` lists every archived commit
with its parents, plus the archive's digest and tip. This module answers the
questions the checkers ask, using git where it can and the ledger where git
cannot:

- ``resolve(rev)``: the full commit id, found in the repository or,
  failing that, by a unique ledger prefix. Otherwise ``None``.
- ``commit_known(rev)``: whether ``resolve`` found it.
- ``is_ancestor(a, b)``: git ancestry when both commits are in the
  repository, and ledger ancestry when both are archived. An archived
  commit is an ancestor of every public commit exactly when it is an
  ancestor (or the tip) of the archived tip, because the public root
  continues the tip. The public commit must descend from that root (pinned
  in ``evidence/history/public-root.json``), so an unrelated orphan commit
  never inherits the archive. A public commit is never an ancestor of an
  archived one.

Nothing here weakens a check. A commit that is neither in the repository
nor in the ledger stays unknown, and an archived commit on a side branch
that never reached the tip is not an ancestor of the public history.
"""

from __future__ import annotations

import functools
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "evidence/history/pre-public-history.json"
# The public root commit, pinned by the first commit after it (a commit
# cannot name itself). Until the pin exists, the repository must have
# exactly one root commit. Resolved against ROOT at call time.
PUBLIC_ROOT_PATH = "evidence/history/public-root.json"
HEX = re.compile(r"^[0-9a-f]{7,40}$")


def _git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True, check=False)


@functools.lru_cache(maxsize=1)
def ledger() -> dict:
    if not LEDGER.exists():
        return {"commits": {}, "tip": None}
    return json.loads(LEDGER.read_text(encoding="utf-8"))


def archived_commits() -> dict[str, list[str]]:
    return ledger().get("commits", {})


def archived_tip() -> str | None:
    return ledger().get("tip")


def _in_repo(rev: str) -> str | None:
    completed = _git("rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}")
    return completed.stdout.strip() if completed.returncode == 0 and completed.stdout.strip() else None


def _in_ledger(rev: str) -> str | None:
    rev = rev.lower()
    if not HEX.match(rev):
        return None
    commits = archived_commits()
    if len(rev) == 40:
        return rev if rev in commits else None
    matches = [sha for sha in commits if sha.startswith(rev)]
    return matches[0] if len(matches) == 1 else None


def resolve(rev: str) -> str | None:
    """Full commit id from the repository, else from a unique ledger prefix."""
    if not rev:
        return None
    return _in_repo(rev) or _in_ledger(rev)


def commit_known(rev: str) -> bool:
    return resolve(rev) is not None


def is_archived(rev: str) -> bool:
    """True when the commit exists only in the pre-public archive."""
    return _in_repo(rev) is None and _in_ledger(rev) is not None


def public_root() -> str | None:
    """The public root commit: pinned once a descendant records it, else HEAD's single root.

    A commit cannot name its own id, so the root is derived until the first
    later commit pins it. More than one parentless ancestor is ambiguous and
    yields None.
    """
    pin = ROOT / PUBLIC_ROOT_PATH
    if pin.exists():
        pinned = json.loads(pin.read_text(encoding="utf-8")).get("public_root", "")
        return _in_repo(pinned) if pinned else None
    roots = _git("rev-list", "--max-parents=0", "HEAD")
    candidates = roots.stdout.split() if roots.returncode == 0 else []
    return candidates[0] if len(candidates) == 1 else None


def _continues_archive(public_sha: str) -> bool:
    """Whether a public commit descends from the root that continues the archive."""
    root = public_root()
    return bool(root) and _git("merge-base", "--is-ancestor", root, public_sha).returncode == 0


@functools.lru_cache(maxsize=None)
def _archived_ancestors(sha: str) -> frozenset[str]:
    """``sha`` and every archived commit reachable from it through parents."""
    commits = archived_commits()
    seen: set[str] = set()
    stack = [sha]
    while stack:
        current = stack.pop()
        if current in seen or current not in commits:
            continue
        seen.add(current)
        stack.extend(commits[current])
    return frozenset(seen)


def is_ancestor(ancestor: str, descendant: str) -> bool:
    """``git merge-base --is-ancestor`` semantics across the history cut."""
    a_repo, b_repo = _in_repo(ancestor), _in_repo(descendant)
    if a_repo and b_repo:
        return _git("merge-base", "--is-ancestor", a_repo, b_repo).returncode == 0
    a_arch = None if a_repo else _in_ledger(ancestor)
    b_arch = None if b_repo else _in_ledger(descendant)
    if a_arch and b_arch:
        return a_arch in _archived_ancestors(b_arch)
    if a_arch and b_repo:
        tip = archived_tip()
        return bool(tip) and a_arch in _archived_ancestors(tip) and _continues_archive(b_repo)
    return False


if __name__ == "__main__":
    import sys

    if len(sys.argv) == 3:
        print(json.dumps({"ancestor": sys.argv[1], "descendant": sys.argv[2],
                          "is_ancestor": is_ancestor(sys.argv[1], sys.argv[2])}))
    else:
        data = ledger()
        print(json.dumps({"archived_commits": len(data.get("commits", {})), "tip": data.get("tip")}))

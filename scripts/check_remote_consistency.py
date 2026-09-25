#!/usr/bin/env python3
"""GitHub consistency gate (SLEY-2.0-ARCHITECTURE-TIGHTENING section 16).

Reports LOCAL_HEAD, UPSTREAM_HEAD, AHEAD_COUNT, BEHIND_COUNT, DIRTY_STATE and
UNTRACKED_RELEVANT_FILES for one branch and judges the handoff state. It
never pushes, fetches, or mutates the repository; pushing stays an explicit
operator or agent action.

Expected handoff state is DIRTY_STATE = CLEAN, BEHIND_COUNT = 0 and
AHEAD_COUNT = 0. AHEAD_COUNT > 0 passes only with --allow-ahead, which is the
explicit declaration that the unpushed work is not being offered for remote
review. A branch whose upstream shares no merge base is reported as
HISTORY_RELATED = false and always fails: nothing on the remote can be cited
as this branch's truth.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def git(repo: Path, *args: str, check: bool = True) -> str:
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        check=False,
    )
    if check and proc.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {proc.stderr.strip()}")
    return proc.stdout.strip()


def inspect(repo: Path, branch: str | None, upstream: str | None, allow_ahead: bool) -> dict:
    branch = branch or git(repo, "rev-parse", "--abbrev-ref", "HEAD")
    local_head = git(repo, "rev-parse", branch)
    report: dict = {
        "REPO": str(repo),
        "BRANCH": branch,
        "LOCAL_HEAD": local_head,
        "UPSTREAM_REF": None,
        "UPSTREAM_HEAD": None,
        "MERGE_BASE": None,
        "HISTORY_RELATED": False,
        "AHEAD_COUNT": None,
        "BEHIND_COUNT": None,
        "DIRTY_STATE": None,
        "DIRTY_PATHS": [],
        "UNTRACKED_RELEVANT_FILES": [],
        "ALLOW_AHEAD": allow_ahead,
        "REASONS": [],
        "RESULT": None,
    }

    upstream_ref = upstream
    if upstream_ref is None:
        upstream_ref = git(
            repo,
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            f"{branch}@{{upstream}}",
            check=False,
        )
    if not upstream_ref:
        report["REASONS"].append("NO_UPSTREAM")
    else:
        report["UPSTREAM_REF"] = upstream_ref
        upstream_head = git(repo, "rev-parse", "--verify", f"{upstream_ref}^{{commit}}", check=False)
        if not upstream_head:
            report["REASONS"].append("UPSTREAM_UNRESOLVED")
        else:
            report["UPSTREAM_HEAD"] = upstream_head
            base = git(repo, "merge-base", local_head, upstream_head, check=False)
            report["MERGE_BASE"] = base or None
            report["HISTORY_RELATED"] = bool(base)
            counts = git(repo, "rev-list", "--left-right", "--count", f"{local_head}...{upstream_head}")
            ahead, behind = (int(x) for x in counts.split())
            report["AHEAD_COUNT"] = ahead
            report["BEHIND_COUNT"] = behind
            if not base:
                report["REASONS"].append("UNRELATED_HISTORY")
            if behind > 0:
                report["REASONS"].append("BEHIND_UPSTREAM")
            if ahead > 0 and not allow_ahead:
                report["REASONS"].append("AHEAD_UNDECLARED")

    # Porcelain v1: XY path. Untracked rows are "?? path"; ignored files are
    # not listed, so anything reported is relevant by definition.
    status = git(repo, "status", "--porcelain", "--untracked-files=all")
    for line in status.splitlines():
        code, path = line[:2], line[3:]
        if code == "??":
            report["UNTRACKED_RELEVANT_FILES"].append(path)
        else:
            report["DIRTY_PATHS"].append(path)
    report["DIRTY_STATE"] = "CLEAN" if not report["DIRTY_PATHS"] else "DIRTY"
    if report["DIRTY_PATHS"]:
        report["REASONS"].append("DIRTY_TREE")
    if report["UNTRACKED_RELEVANT_FILES"]:
        report["REASONS"].append("UNTRACKED_FILES")

    report["RESULT"] = "PASS" if not report["REASONS"] else "FAIL"
    return report


def render_text(report: dict) -> str:
    keys = [
        "BRANCH",
        "LOCAL_HEAD",
        "UPSTREAM_REF",
        "UPSTREAM_HEAD",
        "MERGE_BASE",
        "HISTORY_RELATED",
        "AHEAD_COUNT",
        "BEHIND_COUNT",
        "DIRTY_STATE",
        "UNTRACKED_RELEVANT_FILES",
        "REASONS",
        "RESULT",
    ]
    lines = []
    for key in keys:
        value = report[key]
        if isinstance(value, list):
            value = ", ".join(value) if value else "-"
        lines.append(f"{key} = {value}")
    return "\n".join(lines)


def _run(repo: Path, *args: str, env: dict | None = None) -> None:
    subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, env=env)


def self_test() -> int:
    """Exercise the gate against throwaway repositories; no network."""
    env = dict(os.environ)
    env.update(
        {
            "GIT_AUTHOR_NAME": "gate",
            "GIT_AUTHOR_EMAIL": "gate@example.invalid",
            "GIT_COMMITTER_NAME": "gate",
            "GIT_COMMITTER_EMAIL": "gate@example.invalid",
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_NOSYSTEM": "1",
        }
    )
    failures: list[str] = []

    def expect(name: str, cond: bool) -> None:
        if not cond:
            failures.append(name)

    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        remote = tmp_path / "remote.git"
        _run(tmp_path, "init", "--bare", "-b", "main", str(remote), env=env)
        work = tmp_path / "work"
        _run(tmp_path, "clone", "-q", str(remote), str(work), env=env)
        _run(work, "checkout", "-q", "-b", "main", env=env)
        (work / "a.txt").write_text("one\n")
        _run(work, "add", "a.txt", env=env)
        _run(work, "commit", "-q", "-m", "one", env=env)
        _run(work, "push", "-q", "-u", "origin", "main", env=env)

        r = inspect(work, "main", None, False)
        expect("clean_pushed_pass", r["RESULT"] == "PASS" and r["AHEAD_COUNT"] == 0 and r["BEHIND_COUNT"] == 0)
        expect("clean_pushed_related", r["HISTORY_RELATED"] is True and r["MERGE_BASE"] == r["LOCAL_HEAD"])

        (work / "a.txt").write_text("two\n")
        r = inspect(work, "main", None, False)
        expect("dirty_fail", r["RESULT"] == "FAIL" and r["DIRTY_STATE"] == "DIRTY" and "DIRTY_TREE" in r["REASONS"])

        _run(work, "commit", "-q", "-am", "two", env=env)
        r = inspect(work, "main", None, False)
        expect("ahead_undeclared_fail", r["RESULT"] == "FAIL" and r["AHEAD_COUNT"] == 1 and "AHEAD_UNDECLARED" in r["REASONS"])
        r = inspect(work, "main", None, True)
        expect("ahead_declared_pass", r["RESULT"] == "PASS" and r["AHEAD_COUNT"] == 1)

        (work / "scratch.txt").write_text("x\n")
        r = inspect(work, "main", None, True)
        expect("untracked_fail", r["RESULT"] == "FAIL" and r["UNTRACKED_RELEVANT_FILES"] == ["scratch.txt"])
        (work / "scratch.txt").unlink()

        # A second clone pushes so the first is behind.
        other = tmp_path / "other"
        _run(tmp_path, "clone", "-q", str(remote), str(other), env=env)
        (other / "b.txt").write_text("b\n")
        _run(other, "add", "b.txt", env=env)
        _run(other, "commit", "-q", "-m", "b", env=env)
        _run(other, "push", "-q", "origin", "main", env=env)
        _run(work, "fetch", "-q", "origin", env=env)
        r = inspect(work, "main", None, True)
        expect("behind_fail", r["RESULT"] == "FAIL" and r["BEHIND_COUNT"] == 1 and "BEHIND_UPSTREAM" in r["REASONS"])

        # Unrelated history: a fresh root pushed over the remote branch.
        orphan = tmp_path / "orphan"
        _run(tmp_path, "init", "-q", "-b", "main", str(orphan), env=env)
        (orphan / "c.txt").write_text("c\n")
        _run(orphan, "add", "c.txt", env=env)
        _run(orphan, "commit", "-q", "-m", "root2", env=env)
        _run(orphan, "remote", "add", "origin", str(remote), env=env)
        _run(orphan, "push", "-q", "--force", "origin", "main", env=env)
        _run(orphan, "branch", "-q", "--set-upstream-to=origin/main", "main", env=env)
        _run(work, "fetch", "-q", "origin", env=env)
        r = inspect(work, "main", None, True)
        expect("unrelated_fail", r["RESULT"] == "FAIL" and r["HISTORY_RELATED"] is False and r["MERGE_BASE"] is None and "UNRELATED_HISTORY" in r["REASONS"])

        # No upstream at all.
        _run(work, "checkout", "-q", "-b", "lonely", env=env)
        r = inspect(work, "lonely", None, True)
        expect("no_upstream_fail", r["RESULT"] == "FAIL" and "NO_UPSTREAM" in r["REASONS"])

    if failures:
        print("SELF_TEST FAIL: " + ", ".join(failures))
        return 1
    print("SELF_TEST PASS: 8 cases")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", type=Path, default=ROOT, help="repository or worktree path (default: this repository)")
    parser.add_argument("--branch", help="branch to inspect (default: checked-out branch)")
    parser.add_argument("--upstream", help="upstream ref to compare against (default: the branch's configured upstream)")
    parser.add_argument("--allow-ahead", action="store_true", help="declare unpushed commits as intentionally not offered for remote review")
    parser.add_argument("--json", action="store_true", help="emit the report as JSON")
    parser.add_argument("--self-test", action="store_true", help="run the built-in scenarios against temporary repositories")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    report = inspect(args.repo.resolve(), args.branch, args.upstream, args.allow_ahead)
    print(json.dumps(report, indent=1, sort_keys=True) if args.json else render_text(report))
    return 0 if report["RESULT"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Rerun the full S20-530 contract checker at the accepted state (ADR-0024).

`scripts/check_s20_530_crash_recovery.py` binds every non-narrative workspace
path to the validated commit and is therefore authoritative only at the
accepted state. This script clones the repository into an isolated working
directory, checks out the confirmation commit on a local `main` branch, writes
the exact `.git/config` and `.git/info/exclude` bytes the frozen checker
requires (pinned here so a later change to the live repository configuration
cannot break verification of a historical fact), and runs the no-argument
checker there. The run takes about 95 to 130 minutes on
the workstation and prints nothing until it finishes.

Usage:
    python3 scripts/verify_s20_530_accepted_state.py [--workdir DIR]
        [--commit SHA] [--keep]

`--commit` exists only for an operator-ordered re-validation at a later
accepted commit; the default is the recorded confirmation commit. The clone is
removed after a PASS unless `--keep` is given and is always kept after a FAIL.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GIT = "/usr/bin/git"
PYTHON = "/usr/bin/python3"
CHECKER = "scripts/check_s20_530_crash_recovery.py"
CONFIRMATION_COMMIT = "cc0f92f0b3ff41f3f8ca5db86117619255ddac5c"
FROZEN_GIT_CONFIG = (
    b"[core]\n"
    b"\trepositoryformatversion = 0\n"
    b"\tfilemode = true\n"
    b"\tbare = false\n"
    b"\tlogallrefupdates = true\n"
    b'[remote "origin"]\n'
    b"\turl = https://github.com/GreyforgeLabs/sley.git\n"
    b"\tfetch = +refs/heads/*:refs/remotes/origin/*\n"
    b'[branch "main"]\n'
    b"\tremote = origin\n"
    b"\tmerge = refs/heads/main\n"
)
FROZEN_GIT_INFO_EXCLUDE = (
    b"# git ls-files --others --exclude-from=.git/info/exclude\n"
    b"# Lines that start with '#' are comments.\n"
    b"# For a project mostly in C, the following would be a good set of\n"
    b"# exclude patterns (uncomment them if you want to use them):\n"
    b"# *.[oa]\n"
    b"# *~\n"
)
EXPECTED_PASS_LINE = (
    "S20-530 crash-recovery contract check: PASS "
    "(100 exact matrix rows; implementation_complete=True)"
)


GIT_ENVIRONMENT = {
    **{key: value for key, value in os.environ.items() if not key.startswith("GIT_")},
    "GIT_CONFIG_GLOBAL": "/dev/null",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_TERMINAL_PROMPT": "0",
}


def run(arguments: tuple[str, ...], *, cwd: Path) -> None:
    result = subprocess.run(arguments, cwd=cwd, check=False, env=GIT_ENVIRONMENT)
    if result.returncode != 0:
        print(f"S20-530 accepted-state verification: FAIL: {' '.join(arguments)}")
        raise SystemExit(1)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workdir", type=Path, default=None)
    parser.add_argument("--commit", default=CONFIRMATION_COMMIT)
    parser.add_argument("--keep", action="store_true")
    arguments = parser.parse_args()

    if arguments.workdir is None:
        workdir = Path(tempfile.mkdtemp(prefix="sley2-s20-530-verify-"))
    else:
        workdir = arguments.workdir.resolve()
        workdir.mkdir(parents=True, exist_ok=True)
    clone = workdir / "sley2"
    if clone.exists():
        print(f"S20-530 accepted-state verification: FAIL: {clone} already exists")
        return 1

    print(f"S20-530 accepted-state verification: clone {clone} at {arguments.commit}")
    run((GIT, "clone", "--quiet", "--no-hardlinks", str(ROOT), str(clone)), cwd=ROOT)
    run((GIT, "checkout", "--quiet", "-B", "main", arguments.commit), cwd=clone)
    (clone / ".git/config").write_bytes(FROZEN_GIT_CONFIG)
    os.chmod(clone / ".git/config", 0o644)
    (clone / ".git/info").mkdir(exist_ok=True)
    (clone / ".git/info/exclude").write_bytes(FROZEN_GIT_INFO_EXCLUDE)
    os.chmod(clone / ".git/info/exclude", 0o644)

    sys.stdout.flush()
    result = subprocess.run(
        (PYTHON, "-I", "-B", CHECKER),
        cwd=clone,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    output = result.stdout.decode("utf-8", errors="replace")
    sys.stdout.write(output)
    if result.returncode != 0 or EXPECTED_PASS_LINE not in output:
        print(
            "S20-530 accepted-state verification: FAIL "
            f"(exit {result.returncode}; clone kept at {clone})"
        )
        return 1
    if not arguments.keep:
        shutil.rmtree(workdir if arguments.workdir is None else clone)
    print(
        "S20-530 accepted-state verification: PASS "
        f"(checker PASS at {arguments.commit[:7]} in an isolated clone)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

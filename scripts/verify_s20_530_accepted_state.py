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
import json
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
CLOSEOUT_EVIDENCE = "evidence/validation/s20-530-crash-recovery-closeout-v1.json"
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
    # The frozen checker compares every working-tree file's mode with the
    # committed blob mode (0644 or 0755); clone under umask 022 so the
    # checkout reproduces those modes regardless of the workstation umask,
    # then restore the recorded Git-authority directory modes below.
    os.umask(0o022)
    run((GIT, "clone", "--quiet", "--no-hardlinks", str(ROOT), str(clone)), cwd=ROOT)
    run((GIT, "checkout", "--quiet", "-B", "main", arguments.commit), cwd=clone)
    (clone / ".git/config").write_bytes(FROZEN_GIT_CONFIG)
    (clone / ".git/info").mkdir(exist_ok=True)
    (clone / ".git/info/exclude").write_bytes(FROZEN_GIT_INFO_EXCLUDE)
    # The frozen checker compares the live Git local-authority record (owner,
    # directory and file modes) with the record captured at validation, so the
    # clone must reproduce those modes exactly; apply them from the frozen
    # closeout evidence at the checked-out commit and fail early on any
    # directory mode the clone cannot reproduce.
    authority = json.loads((clone / CLOSEOUT_EVIDENCE).read_text(encoding="utf-8"))["validation"][
        "git_local_authority"
    ]
    os.chmod(clone / ".git/config", int(authority["config"]["mode"], 8))
    os.chmod(clone / ".git/info/exclude", int(authority["info_exclude"]["mode"], 8))
    for relative, field in (
        (".", "root_mode"),
        (".git", "git_dir_mode"),
        (".git/info", "info_dir_mode"),
        (".git/objects", "objects_dir_mode"),
        (".git/objects/info", "objects_info_dir_mode"),
        (".git/refs", "refs_dir_mode"),
    ):
        os.chmod(clone / relative, int(authority[field], 8))
    if os.geteuid() != authority["owner_uid"] or os.getegid() != authority["owner_gid"]:
        print(
            "S20-530 accepted-state verification: FAIL: the validated Git authority was "
            f"captured by uid {authority['owner_uid']} gid {authority['owner_gid']}; "
            f"this process is uid {os.geteuid()} gid {os.getegid()}"
        )
        return 1

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

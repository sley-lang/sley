#!/usr/bin/env python3
"""S20-530 acceptance anchor (ADR-0024 historical acceptance).

The S20-530 crash-recovery package was accepted at commit 034cc75 (validated
commit 8f7c763) and its full contract checker,
`scripts/check_s20_530_crash_recovery.py`, passed at commit cc0f92f. That
checker binds every non-narrative workspace path to the validated commit, so it
is the authoritative gate only at the accepted state. `make s20-530-verify`
reruns it there in an isolated clone.

This anchor is the `make quick` gate at every later HEAD. It verifies that the
accepted state is intact as immutable history:

1. the validated, acceptance, and confirmation commits are ancestors of HEAD;
2. every frozen S20-530 authority and evidence path at HEAD is the same Git
   blob as at the acceptance commit, and the working-tree bytes equal that blob;
3. the retained final-checker confirmation log has its recorded digest;
4. the machine summary still records the accepted S20-530 facts exactly;
5. the work-package table still carries the S20-530 row markers.

Only read-only Git object commands are used (rev-parse, cat-file, merge-base,
ls-tree); the index is never refreshed. Later development that changes owner
sources is governed by ADR-0024, not by this anchor.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GIT = "/usr/bin/git"

VALIDATED_COMMIT = "8f7c7630c3ba786478aa0066ba35c271feff2036"
ACCEPTANCE_COMMIT = "034cc75abb59d743ea30b2f2a410205a018cb25f"
CONFIRMATION_COMMIT = "cc0f92f0b3ff41f3f8ca5db86117619255ddac5c"
FREEZE_COMMIT = "250557833364ae1f85dc67290ff6c7c5f0c7fdba"
CONTRACT_SET_SHA256 = "0257eddda95d24dba657b993eee981443e6c215e93f7cb04387a8d444a9c7caa"
SOURCE_SET_SHA256 = "468490e1b67360344fd8384a71be3861b650eb6004587256c6fa70c859cc2ab5"
FREEZE_EVIDENCE = "evidence/validation/s20-530-crash-recovery-contract-freeze-v13.json"
CLOSEOUT_EVIDENCE = "evidence/validation/s20-530-crash-recovery-closeout-v1.json"
TEST_PLAN = "evidence/validation/s20-530-crash-recovery-test-plan-v1.json"
LOG_DIR = "evidence/validation/s20-530-crash-recovery-logs-v1"
LOG_COUNT = 28
CONFIRMATION_LOG = (
    "machineresearch/sley-2.0/s20-530-final-checker-v13-confirmation-2026-09-02.log"
)
CONFIRMATION_LOG_SHA256 = (
    "f8e93918697c73ea91a4df1a74435a36ab0c3915d2da8b4818735d1ac39a8cdd"
)
CONFIRMATION_PASS_LINE = (
    "S20-530 crash-recovery contract check: PASS "
    "(100 exact matrix rows; implementation_complete=True)"
)
SUMMARY = "machineresearch/sley-2.0/machine-summary.json"
WORK_PACKAGES = "docs/WORK_PACKAGES.md"

FROZEN_AUTHORITY_PATHS = (
    "docs/spec/CRASH_RECOVERY_MATRIX_V1.md",
    "docs/adr/ADR-0023-crash-recovery-boundary.md",
    "scripts/check_s20_530_crash_recovery.py",
    "scripts/run_s20_530_validation.py",
    "scripts/reconcile_s20_530_exception_ledgers.py",
    FREEZE_EVIDENCE,
    CLOSEOUT_EVIDENCE,
    TEST_PLAN,
)

CONTRACT_REVIEW_SESSIONS = {
    "nabu": "forge-nabu-s20-530-contract-freeze-20260902T173505-4e2f40e0",
    "ariadne": "forge-ariadne-s20-530-contract-freeze-20260902T173505-ee0cd3fa",
    "vulcan": "forge-vulcan-s20-530-contract-freeze-20260902T173505-d7eb9f55",
}
IMPLEMENTATION_REVIEW_SESSIONS = {
    "nabu": "forge-nabu-s20-530-implementation-20260902T215406-23c1f91b",
    "ariadne": "forge-ariadne-s20-530-implementation-20260902T215406-3ae4faa7",
    "vulcan": "forge-vulcan-s20-530-implementation-20260902T215406-33df6034",
}


def fail(reason: str) -> None:
    print(f"S20-530 acceptance anchor: FAIL: {reason}")
    raise SystemExit(1)


def git(*arguments: str) -> bytes:
    result = subprocess.run(
        (GIT, *arguments),
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        fail(f"git {' '.join(arguments)} failed: {detail}")
    return result.stdout


def require_ancestor(commit: str, label: str) -> None:
    result = subprocess.run(
        (GIT, "merge-base", "--is-ancestor", commit, "HEAD"),
        cwd=ROOT,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        fail(f"{label} {commit} is not an ancestor of HEAD")


def blob_id(revision: str, relative: str) -> str:
    return git("rev-parse", "--verify", f"{revision}:{relative}").decode("ascii").strip()


def tree_paths(revision: str, directory: str) -> list[str]:
    raw = git("ls-tree", "-r", "-z", "--name-only", revision, "--", directory)
    return sorted(value.decode("utf-8") for value in raw.split(b"\0") if value)


def working_tree_sha256(relative: str) -> str:
    path = ROOT / relative
    if path.is_symlink() or not path.is_file():
        fail(f"{relative} is not a regular working-tree file")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_frozen(relative: str) -> None:
    accepted = blob_id(ACCEPTANCE_COMMIT, relative)
    current = blob_id("HEAD", relative)
    if accepted != current:
        fail(f"{relative} changed after the acceptance commit")
    committed = git("cat-file", "blob", current)
    if working_tree_sha256(relative) != hashlib.sha256(committed).hexdigest():
        fail(f"{relative} working-tree bytes differ from HEAD")


def main() -> None:
    for commit, label in (
        (VALIDATED_COMMIT, "validated commit"),
        (ACCEPTANCE_COMMIT, "acceptance commit"),
        (CONFIRMATION_COMMIT, "confirmation commit"),
    ):
        require_ancestor(commit, label)
    freeze = git("rev-parse", "--verify", f"{FREEZE_COMMIT}^{{commit}}").decode().strip()
    if freeze != FREEZE_COMMIT:
        fail("freeze commit is missing")

    logs = tree_paths(ACCEPTANCE_COMMIT, LOG_DIR)
    if len(logs) != LOG_COUNT:
        fail(f"acceptance commit retains {len(logs)} logs, expected {LOG_COUNT}")
    if tree_paths("HEAD", LOG_DIR) != logs:
        fail("retained log set differs from the acceptance commit")
    for relative in (*FROZEN_AUTHORITY_PATHS, *logs):
        require_frozen(relative)

    if working_tree_sha256(CONFIRMATION_LOG) != CONFIRMATION_LOG_SHA256:
        fail("final-checker confirmation log digest differs")
    confirmation = (ROOT / CONFIRMATION_LOG).read_text(encoding="utf-8")
    if CONFIRMATION_PASS_LINE not in confirmation or "CHECKER_EXIT=0" not in confirmation:
        fail("final-checker confirmation log lacks the PASS record")

    summary = json.loads((ROOT / SUMMARY).read_text(encoding="utf-8"))
    package = summary.get("s20_530_crash_recovery")
    if not isinstance(package, dict):
        fail("machine summary lacks s20_530_crash_recovery")
    expected_scalars = {
        "status": "CONTRACT_FROZEN_IMPLEMENTATION_COMPLETE",
        "contract": FROZEN_AUTHORITY_PATHS[0],
        "adr": FROZEN_AUTHORITY_PATHS[1],
        "matrix_rows": 100,
        "dependency_direction": "sley-repo -> sley-txn -> sley-store",
        "implementation_complete": True,
        "contract_freeze_evidence": FREEZE_EVIDENCE,
        "contract_set_sha256": CONTRACT_SET_SHA256,
        "freeze_commit": FREEZE_COMMIT,
        "validation_evidence": CLOSEOUT_EVIDENCE,
        "validated_commit": VALIDATED_COMMIT,
        "source_set_sha256": SOURCE_SET_SHA256,
        "acceptance_commit": ACCEPTANCE_COMMIT,
        "final_checker_confirmation_commit": CONFIRMATION_COMMIT,
        "final_checker_confirmation_log": CONFIRMATION_LOG,
        "aging_rule": "docs/adr/ADR-0024-accepted-package-aging.md",
    }
    for field, expected in expected_scalars.items():
        actual = package.get(field)
        if type(actual) is not type(expected) or actual != expected:
            fail(f"machine summary {field} differs")
    for field, sessions, verdict in (
        ("contract_reviews", CONTRACT_REVIEW_SESSIONS, "PASS_CONTRACT_FREEZE"),
        ("implementation_reviews", IMPLEMENTATION_REVIEW_SESSIONS, "PASS_IMPLEMENTATION"),
    ):
        reviews = package.get(field)
        if not isinstance(reviews, dict) or tuple(reviews) != tuple(sessions):
            fail(f"machine summary {field} roles/order differ")
        for role, session_id in sessions.items():
            review = reviews[role]
            if not isinstance(review, dict):
                fail(f"machine summary {field}.{role} is not an object")
            if review.get("result") != verdict:
                fail(f"machine summary {field}.{role} result differs")
            if review.get("session_id") != session_id:
                fail(f"machine summary {field}.{role} session differs")
            if review.get("contract_set_sha256") != CONTRACT_SET_SHA256:
                fail(f"machine summary {field}.{role} contract set differs")

    work_packages = (ROOT / WORK_PACKAGES).read_text(encoding="utf-8")
    for marker in ("| S20-530 |", "100-row crash matrix", "ADR-0024"):
        if marker not in work_packages:
            fail(f"work-package table lacks {marker!r}")

    print(
        "S20-530 acceptance anchor: PASS "
        f"(validated {VALIDATED_COMMIT[:7]}, accepted {ACCEPTANCE_COMMIT[:7]}, "
        f"confirmed {CONFIRMATION_COMMIT[:7]}; "
        f"{len(FROZEN_AUTHORITY_PATHS)} frozen authority paths, {LOG_COUNT} logs)"
    )


if __name__ == "__main__":
    if len(sys.argv) != 1:
        fail("usage: check_s20_530_acceptance_anchor.py")
    main()

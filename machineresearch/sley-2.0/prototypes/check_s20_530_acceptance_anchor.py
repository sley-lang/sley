#!/usr/bin/env python3
"""S20-530 acceptance anchor (Option A prototype, not yet in the repository).

Verifies at HEAD that the accepted S20-530 crash-recovery closeout is intact
as immutable history, without re-binding the whole workspace:

1. the validated commit and the acceptance commit are ancestors of HEAD;
2. every frozen S20-530 authority and evidence path at HEAD is the same Git
   blob as at the acceptance commit, and the working-tree bytes equal that blob;
3. the machine summary still records the accepted S20-530 facts exactly;
4. the work-package table still carries the S20-530 row markers.

Only read-only Git object commands are used (rev-parse, cat-file, merge-base,
ls-tree); the index is never refreshed.

The full contract checker, scripts/check_s20_530_crash_recovery.py, remains
the authoritative historical gate and is run at the acceptance-confirmed
commit by `make s20-530-verify` (isolated clone).
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
FREEZE_COMMIT = "250557833364ae1f85dc67290ff6c7c5f0c7fdba"
CONTRACT_SET_SHA256 = "0257eddda95d24dba657b993eee981443e6c215e93f7cb04387a8d444a9c7caa"
SOURCE_SET_SHA256 = "468490e1b67360344fd8384a71be3861b650eb6004587256c6fa70c859cc2ab5"
FREEZE_EVIDENCE = "evidence/validation/s20-530-crash-recovery-contract-freeze-v13.json"
CLOSEOUT_EVIDENCE = "evidence/validation/s20-530-crash-recovery-closeout-v1.json"
TEST_PLAN = "evidence/validation/s20-530-crash-recovery-test-plan-v1.json"
LOG_DIR = "evidence/validation/s20-530-crash-recovery-logs-v1"
LOG_COUNT = 28
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
        fail(f"git {' '.join(arguments)} failed: {result.stderr.decode(errors='replace').strip()}")
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


def require_frozen(relative: str) -> None:
    accepted = blob_id(ACCEPTANCE_COMMIT, relative)
    current = blob_id("HEAD", relative)
    if accepted != current:
        fail(f"{relative} changed after the acceptance commit")
    path = ROOT / relative
    if path.is_symlink() or not path.is_file():
        fail(f"{relative} is not a regular working-tree file")
    committed = git("cat-file", "blob", current)
    if hashlib.sha256(path.read_bytes()).digest() != hashlib.sha256(committed).digest():
        fail(f"{relative} working-tree bytes differ from HEAD")


def main() -> None:
    require_ancestor(VALIDATED_COMMIT, "validated commit")
    require_ancestor(ACCEPTANCE_COMMIT, "acceptance commit")
    if git("rev-parse", "--verify", f"{FREEZE_COMMIT}^{{commit}}").decode().strip() != FREEZE_COMMIT:
        fail("freeze commit is missing")

    logs = sorted(
        line
        for line in git("ls-tree", "-r", "--name-only", ACCEPTANCE_COMMIT, "--", LOG_DIR)
        .decode("utf-8")
        .splitlines()
        if line
    )
    if len(logs) != LOG_COUNT:
        fail(f"acceptance commit retains {len(logs)} logs, expected {LOG_COUNT}")
    current_logs = sorted(
        line
        for line in git("ls-tree", "-r", "--name-only", "HEAD", "--", LOG_DIR)
        .decode("utf-8")
        .splitlines()
        if line
    )
    if current_logs != logs:
        fail("retained log set differs from the acceptance commit")
    for relative in (*FROZEN_AUTHORITY_PATHS, *logs):
        require_frozen(relative)

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
    for marker in ("| S20-530 |", "100-row crash matrix"):
        if marker not in work_packages:
            fail(f"work-package table lacks {marker!r}")

    print(
        "S20-530 acceptance anchor: PASS "
        f"(validated {VALIDATED_COMMIT[:7]}, accepted {ACCEPTANCE_COMMIT[:7]}, "
        f"{len(FROZEN_AUTHORITY_PATHS)} frozen authority paths, {LOG_COUNT} logs)"
    )


if __name__ == "__main__":
    if len(sys.argv) != 1:
        fail("usage: check_s20_530_acceptance_anchor.py")
    main()

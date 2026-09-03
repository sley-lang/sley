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
4. the machine summary still records the accepted S20-530 facts exactly,
   including the six receipt rows byte-for-byte as in the frozen evidence;
   all 419 mapped tests still carry `#[test]` in their owner crates; and
   ADR-0024 and the verify script match the digests pinned here;
5. the work-package table and the closeout audit still carry the S20-530
   fact markers, and the Makefile still registers this anchor in `quick` and
   the `s20-530-verify` target.

Only read-only Git object commands are used (rev-parse, cat-file, merge-base,
ls-tree); the index is never refreshed. Later development that changes owner
sources is governed by ADR-0024, not by this anchor.
"""

from __future__ import annotations

import hashlib
import json
import re
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
CLOSEOUT_AUDIT = "docs/audits/S20_530_CRASH_RECOVERY_CLOSEOUT.md"
CLOSEOUT_AUDIT_MARKERS = (
    "S20-530 crash recovery complete under the v13 contract",
    "`0257eddda95d24dba657b993eee981443e6c215e93f7cb04387a8d444a9c7caa`",
    "validated commit `8f7c7630c3ba786478aa0066ba35c271feff2036`",
    "commit `034cc75abb59d743ea30b2f2a410205a018cb25f` records",
    "at commit `cc0f92f0b3ff41f3f8ca5db86117619255ddac5c` printed",
    "(rows=100, tests=419)",
    "with 28 retained logs",
)
MAKEFILE = "Makefile"
MAPPED_TEST_COUNT = 419
# The anchor's own trust base: these files are pinned by digest here, so a
# change to the aging rule or the historical verification path is always an
# explicit, reviewed edit of this anchor as well.
SELF_PINNED_PATHS = {
    "docs/adr/ADR-0024-accepted-package-aging.md": "09e13f500f433ae228b843a92dbdcf057e33bde7a874659051b7958d5c1ae31a",
    "scripts/verify_s20_530_accepted_state.py": "eddc6d77758055c6852f28aa2c2166088c61606f4b1084184dac8ee8bd71613c",
}
MAKEFILE_MARKERS = (
    "\tpython3 scripts/check_s20_530_acceptance_anchor.py\n",
    "\ns20-530-verify:\n\tpython3 scripts/verify_s20_530_accepted_state.py\n",
)

FROZEN_AUTHORITY_PATHS = (
    "docs/spec/CRASH_RECOVERY_MATRIX_V1.md",
    "docs/adr/ADR-0023-crash-recovery-boundary.md",
    "scripts/check_s20_530_crash_recovery.py",
    "scripts/run_s20_530_validation.py",
    "scripts/reconcile_s20_530_exception_ledgers.py",
    "scripts/build_s20_530_test_plan.py",
    "scripts/render_s20_530_grouped_m2.py",
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


def tree_mode(revision: str, relative: str) -> str:
    raw = git("ls-tree", "-z", revision, "--", relative)
    entries = [value for value in raw.split(b"\0") if value]
    if len(entries) != 1:
        fail(f"{relative} is not exactly one tree entry at {revision[:7]}")
    return entries[0].split(b" ", 1)[0].decode("ascii")


def require_frozen(relative: str) -> None:
    accepted = blob_id(ACCEPTANCE_COMMIT, relative)
    current = blob_id("HEAD", relative)
    if accepted != current:
        fail(f"{relative} changed after the acceptance commit")
    accepted_mode = tree_mode(ACCEPTANCE_COMMIT, relative)
    if accepted_mode not in ("100644", "100755"):
        fail(f"{relative} has a non-regular tree mode at the acceptance commit")
    if tree_mode("HEAD", relative) != accepted_mode:
        fail(f"{relative} tree mode changed after the acceptance commit")
    committed = git("cat-file", "blob", current)
    if working_tree_sha256(relative) != hashlib.sha256(committed).hexdigest():
        fail(f"{relative} working-tree bytes differ from HEAD")
    executable = bool((ROOT / relative).stat().st_mode & 0o111)
    if executable != (accepted_mode == "100755"):
        fail(f"{relative} working-tree mode differs from the acceptance commit")


def require_self_pinned(relative: str, expected_sha256: str) -> None:
    if working_tree_sha256(relative) != expected_sha256:
        fail(f"{relative} differs from its pinned digest (update the anchor deliberately)")


TEST_ATTRIBUTE = re.compile(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+([A-Za-z0-9_]+)\s*\(")


def require_mapped_tests_retained(validation: dict) -> None:
    executed = validation.get("executed_tests")
    owners = validation.get("test_owners")
    if not isinstance(executed, list) or not isinstance(owners, dict):
        fail("closeout evidence lacks the executed-test inventory")
    if len(executed) != MAPPED_TEST_COUNT or len(set(executed)) != MAPPED_TEST_COUNT:
        fail(f"closeout evidence lists {len(executed)} mapped tests, expected {MAPPED_TEST_COUNT}")
    present: dict[str, set[str]] = {}
    for crate in sorted(set(owners.values())):
        names: set[str] = set()
        for source in sorted((ROOT / "crates" / crate / "src").rglob("*.rs")):
            if source.is_symlink():
                fail(f"owner source is a symlink: {source.relative_to(ROOT)}")
            names.update(TEST_ATTRIBUTE.findall(source.read_text(encoding="utf-8")))
        present[crate] = names
    missing = sorted(
        name for name in executed if name not in present.get(owners.get(name, ""), set())
    )
    if missing:
        fail(f"mapped tests no longer carry #[test] in their owner crate: {missing[:5]!r}")


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
        "final_checker_confirmation_log_sha256": CONFIRMATION_LOG_SHA256,
        "final_checker_confirmation_finished_at_utc": "2026-09-03T01:06:49Z",
        "closeout_audit": CLOSEOUT_AUDIT,
        "aging_rule": "docs/adr/ADR-0024-accepted-package-aging.md",
        "acceptance_anchor": "scripts/check_s20_530_acceptance_anchor.py",
    }
    for field, expected in expected_scalars.items():
        actual = package.get(field)
        if type(actual) is not type(expected) or actual != expected:
            fail(f"machine summary {field} differs")
    freeze_evidence = json.loads((ROOT / FREEZE_EVIDENCE).read_text(encoding="utf-8"))
    closeout_evidence = json.loads((ROOT / CLOSEOUT_EVIDENCE).read_text(encoding="utf-8"))
    for field, sessions, verdict, evidence in (
        ("contract_reviews", CONTRACT_REVIEW_SESSIONS, "PASS_CONTRACT_FREEZE", freeze_evidence),
        (
            "implementation_reviews",
            IMPLEMENTATION_REVIEW_SESSIONS,
            "PASS_IMPLEMENTATION",
            closeout_evidence,
        ),
    ):
        reviews = package.get(field)
        if not isinstance(reviews, dict) or tuple(reviews) != tuple(sessions):
            fail(f"machine summary {field} roles/order differ")
        if reviews != evidence.get("reviews"):
            fail(f"machine summary {field} differ from the frozen evidence rows")
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
    closeout_audit = (ROOT / CLOSEOUT_AUDIT).read_text(encoding="utf-8")
    for marker in CLOSEOUT_AUDIT_MARKERS:
        if marker not in closeout_audit:
            fail(f"closeout audit lacks {marker!r}")
    validation = closeout_evidence.get("validation")
    if not isinstance(validation, dict):
        fail("closeout evidence lacks validation")
    require_mapped_tests_retained(validation)
    for relative, expected in SELF_PINNED_PATHS.items():
        require_self_pinned(relative, expected)

    makefile = (ROOT / MAKEFILE).read_text(encoding="utf-8")
    quick = makefile.split("\nquick:\n", 1)
    if len(quick) != 2 or MAKEFILE_MARKERS[0] not in quick[1].split("\n\n", 1)[0] + "\n":
        fail("quick gate omits the S20-530 acceptance anchor")
    if MAKEFILE_MARKERS[1] not in makefile:
        fail("Makefile omits the s20-530-verify target")

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

#!/usr/bin/env python3
"""Check the scoped S20-600 frozen legacy artifact adapter."""

from __future__ import annotations

import ast
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from bench.legacy.runner import (  # noqa: E402
    FROZEN_CONTRACT,
    LegacyErrorCode,
    LegacyRunnerError,
    verify_frozen_artifact,
)


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


runner = read("bench/legacy/runner.py")
tests = read("bench/legacy/tests/test_runner.py")
spec = read("docs/spec/LEGACY_ARTIFACT_ADAPTER_V1.md")
adr = read("docs/adr/ADR-0018-frozen-legacy-artifact-adapter.md")
legacy_readme = read("bench/legacy/README.md")
bench_readme = read("bench/README.md")
work_packages = read("docs/WORK_PACKAGES.md")
makefile = read("Makefile")
problems: list[str] = []

for token in [
    "Status: S20-600 verified-artifact and version-smoke contract only.",
    "b24f19c6a348751c93c9cf63f6f4154f6132796112c26f9d8c0e71324080dbc7",
    "1,568 members",
    "1,067 sorted payload entries",
    "NOT_ENFORCED_VERSION_ONLY",
    "Numeric codes 60000 through 60014",
    "Full S20-600 remains open",
]:
    if token not in spec:
        problems.append(f"legacy adapter spec missing {token!r}")

for token in [
    "class LegacyErrorCode(IntEnum)",
    "class LegacyArtifactContract",
    "def verify_frozen_artifact",
    "def staged_frozen_artifact",
    "def run_version_smoke",
    "def record_smoke_evidence",
    'VERSION_ARGUMENTS = ("--version",)',
    "DRAIN_GRACE_SECONDS",
    "O_NOFOLLOW",
    "subprocess.Popen(",
    "shell=False",
    "start_new_session=True",
    '"SLEY_SOURCE_CACHE_DIR"',
    '"network_isolation": "NOT_ENFORCED_VERSION_ONLY"',
    '"stage_write_bits_removed": True',
    '"read_only_mount_enforced": False',
    '"benchmark_trials_executed": 0',
    '"full_s20_600_complete": False',
]:
    if token not in runner:
        problems.append(f"legacy adapter implementation missing {token!r}")

for forbidden in [
    "extractall(",
    ".extract(",
    "shell=True",
    "os.system",
    "os.popen",
    '"/home/greyforge/sley"',
]:
    if forbidden in runner:
        problems.append(f"legacy adapter contains forbidden surface {forbidden!r}")

tree = ast.parse(runner)
imports: set[str] = set()
for node in ast.walk(tree):
    if isinstance(node, ast.Import):
        imports.update(alias.name.split(".")[0] for alias in node.names)
    elif isinstance(node, ast.ImportFrom) and node.module:
        imports.add(node.module.split(".")[0])
for forbidden in {"socket", "requests", "urllib", "http", "ftplib"}:
    if forbidden in imports:
        problems.append(f"legacy adapter imports network module {forbidden!r}")

# Error codes are derived from the enum, never counted as substrings:
# every member must appear with its exact numeric value, and the count
# must match. A removed or renumbered code fails here, not silently.
code_count = len(LegacyErrorCode)
for code in LegacyErrorCode:
    if f"{code.name} = {code.value:_}" not in runner:
        problems.append(f"legacy adapter code drift: {code.name} = {code.value}")
if code_count != 15:
    problems.append(f"legacy adapter code count drift: {code_count}")

# Test coverage is discovered from the suite, never a hardcoded total.
# The token-presence check below is a tripwire only: it proves the code
# name is referenced, while the unittest run underneath proves a test
# actually produces it (a passing suite with a dropped test fails loudly
# on the missing name, not silently on coverage).
test_tree = ast.parse(tests)
discovered = sorted(
    node.name
    for node in ast.walk(test_tree)
    if isinstance(node, ast.FunctionDef) and node.name.startswith("test_")
)
for token in [
    "test_valid_artifact_is_verified_stage_write_bits_removed_and_smoked",
    "test_error_code_enum_is_stable_and_complete",
    "test_missing_artifact_path_reports_missing",
    "test_garbage_archive_reports_invalid",
    "test_archive_ceilings_fail_closed",
    "test_oversized_manifest_fails_closed",
    "test_malformed_and_duplicate_key_manifests_fail_closed",
    "test_manifest_missing_field_fails_closed",
    "test_missing_manifest_file_fails_closed",
    "test_authority_drift_and_tree_digest_drift_fail_closed",
    "test_contamination_member_kinds_fail_closed",
    "test_wrong_top_and_directory_drift_fail_closed",
    "test_unmanifested_payload_and_sley_identity_fail_closed",
    "test_invalid_contract_fails_invariant",
    "test_staging_copy_failure_reports_staging_failed",
    "test_outer_identity_drift_fails_before_archive_use",
    "test_traversal_links_and_duplicates_fail_closed",
    "test_payload_tamper_fails_manifest_inventory_check",
    "test_timeout_is_returned_as_retained_failure",
    "test_hung_child_that_closed_fds_is_a_timeout",
    "test_escaped_grandchild_does_not_spin_the_drain",
    "test_nonzero_exit_and_stderr_are_retained",
    "test_command_failed_reports_symbol",
    "test_output_limit_kills_and_retains_prefix",
    "test_nonfinite_timeout_fails_closed",
    "test_evidence_records_are_create_only",
    "test_mode_hardening_failure_reaches_evidence",
]:
    if token not in tests:
        problems.append(f"legacy adapter tests missing {token!r}")
for code in LegacyErrorCode:
    if f"LegacyErrorCode.{code.name}" not in tests and code.symbol not in tests:
        problems.append(f"legacy adapter code never produced by tests: {code.name}")

if "scoped frozen-artifact adapter" not in bench_readme:
    problems.append("benchmark README omits scoped S20-600 adapter")
if "verified frozen-artifact/version-smoke adapter" not in work_packages:
    problems.append("S20-600 work-package checkpoint is absent")
if "python3 scripts/check_legacy_runner.py" not in makefile:
    problems.append("routine quick gate omits legacy adapter checker")
if "legacy-runner-smoke:" not in makefile:
    problems.append("Makefile omits explicit real legacy smoke target")
if "never reads or writes" not in legacy_readme:
    problems.append("legacy README omits live-checkout boundary")
if "successful longer smoke" not in adr:
    problems.append("legacy ADR omits retained negative and positive evidence")
if "host-local" not in adr:
    problems.append("legacy ADR omits host-local scope of retained evidence")

# FROZEN_CONTRACT is reconciled against the spec's pinned values and the
# register section: sha, size, release, commit, tree digest, sley digest,
# member/file/payload counts. Size and count literals are formatted from
# the contract itself so a drift in runner.py fails here on any host.
for token in [
    FROZEN_CONTRACT.artifact_sha256,
    f"{FROZEN_CONTRACT.artifact_size_bytes:,}",
    FROZEN_CONTRACT.release,
    FROZEN_CONTRACT.source_commit,
    FROZEN_CONTRACT.payload_tree_digest,
    FROZEN_CONTRACT.expected_sley_digest,
    f"{FROZEN_CONTRACT.expected_archive_member_count:,} members",
    f"{FROZEN_CONTRACT.expected_payload_file_count:,} sorted payload entries",
    f"{FROZEN_CONTRACT.expected_payload_total_bytes:,} payload",
]:
    if token not in spec:
        problems.append(f"spec/contract drift: {token!r}")
summary = json.loads(read("machineresearch/sley-2.0/machine-summary.json"))
section = summary.get("s20_600_frozen_legacy_adapter", {})
for key, value in [
    ("artifact_sha256", FROZEN_CONTRACT.artifact_sha256),
    ("artifact_size_bytes", FROZEN_CONTRACT.artifact_size_bytes),
    ("source_commit", FROZEN_CONTRACT.source_commit),
    ("archive_member_count", FROZEN_CONTRACT.expected_archive_member_count),
    ("regular_file_count", FROZEN_CONTRACT.expected_regular_file_count),
    ("payload_file_count", FROZEN_CONTRACT.expected_payload_file_count),
    ("payload_total_bytes", FROZEN_CONTRACT.expected_payload_total_bytes),
    ("successful_version_output", FROZEN_CONTRACT.expected_version_output),
]:
    if section.get(key) != value:
        problems.append(f"register/contract drift: {key}")
if "host-local" not in str(section.get("retained_timeout_evidence_scope", "")):
    problems.append("register omits host-local scope of retained timeout evidence")
if section.get("allowed_command") != ["bin/sley", "--version"]:
    problems.append("register/contract drift: allowed_command")

unit = subprocess.run(
    [sys.executable, "-m", "unittest", "discover", "-s", "bench/legacy/tests"],
    cwd=ROOT,
    check=False,
    capture_output=True,
    text=True,
)
if unit.returncode != 0:
    problems.append(
        f"synthetic legacy adapter tests failed: {(unit.stderr or unit.stdout).strip()}"
    )

# The retained-evidence directory is read, not assumed: its records are
# host-local by mechanism (gitignored create-only files).
evidence_dir = ROOT / "evidence/runtime/s20-600-legacy-smoke"
try:
    retained = sorted(path.name for path in evidence_dir.glob("*.json"))
except OSError as error:
    problems.append(f"retained evidence directory unreadable: {error}")
    retained = []

# Absent artifact is BLOCKED (distinct result), never a verification FAIL:
# the default path is host-local, so a host without the archive must not
# report an identity failure.
missing_probe = ROOT / "evidence/runtime/s20-600-legacy-smoke/does-not-exist.tar.gz"
try:
    verify_frozen_artifact(missing_probe, FROZEN_CONTRACT)
    problems.append("missing artifact verified without error")
except LegacyRunnerError as error:
    if error.code is not LegacyErrorCode.ARTIFACT_MISSING:
        problems.append(f"missing artifact misclassified: {error.code.symbol}")

verified = None
blocked = False
try:
    verified = verify_frozen_artifact()
except LegacyRunnerError as error:
    if error.code is LegacyErrorCode.ARTIFACT_MISSING:
        blocked = True
    else:
        problems.append(f"frozen artifact verification failed: {error}")

if blocked:
    result = {
        "contract": "s20-600-frozen-legacy-artifact-adapter-v1",
        "result": "BLOCKED",
        "reason": "ARTIFACT_MISSING",
        "artifact_bytes_verified": False,
        "stable_error_codes": code_count,
        "synthetic_smoke_tests": len(discovered),
        "retained_evidence_records": retained,
        "problems": problems,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    raise SystemExit(0 if not problems else 1)

result = {
    "contract": "s20-600-frozen-legacy-artifact-adapter-v1",
    "artifact_sha256": FROZEN_CONTRACT.artifact_sha256,
    "artifact_size_bytes": FROZEN_CONTRACT.artifact_size_bytes,
    "artifact_bytes_verified": verified is not None,
    "archive_members_verified": verified.archive_member_count if verified else None,
    "payload_files_verified": verified.payload_file_count if verified else None,
    "payload_bytes_verified": verified.payload_total_bytes if verified else None,
    "stable_error_codes": code_count,
    "synthetic_smoke_tests": len(discovered),
    "retained_evidence_records": retained,
    "manual_real_smoke_target": "make legacy-runner-smoke",
    "real_smoke_in_routine_quick": False,
    "provider_or_model_execution": False,
    "benchmark_trials_executed": 0,
    "network_isolation_claimed": False,
    "live_legacy_checkout_interaction": False,
    "full_s20_600_complete": False,
    "problems": problems,
    "result": "PASS" if not problems else "FAIL",
}
print(json.dumps(result, indent=2, sort_keys=True))
raise SystemExit(0 if not problems else 1)

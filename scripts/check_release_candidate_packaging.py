#!/usr/bin/env python3
"""Check the S20-720 release candidate packaging contract and its stage."""

from __future__ import annotations

import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md"
ADR = ROOT / "docs/adr/ADR-0038-release-candidate-packaging-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
SCRIPT = ROOT / "scripts/build_release_candidate.py"
DEMO = ROOT / "bench/release/run_demo.py"
TESTS = ROOT / "bench/release/tests/test_packaging.py"
FIXTURE = ROOT / "conformance/release-demo/v1/demo.json"

DRAFT_STATUS = "S20_720_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_720_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_720_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_720_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_720_MECHANICS_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_720_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (72000, "PACKAGE_BUILD_FAILED"),
    (72001, "PACKAGE_MANIFEST_INVALID"),
    (72002, "PACKAGE_CONTENT_FORBIDDEN"),
    (72003, "PACKAGE_CONFORMANCE_FAILED"),
    (72004, "PACKAGE_DEMO_FAILED"),
    (72005, "PACKAGE_NOT_REPRODUCIBLE"),
    (72006, "PACKAGE_TREE_DIRTY"),
    (72007, "PACKAGE_INTERNAL_INVARIANT"),
)
SPEC_MARKERS = (
    "# Release Candidate Packaging v1",
    "Status: S20-720 contract draft",
    "## 1. Build",
    "`--remap-path-prefix` mapping the working tree to `/sley2`",
    "## 2. Contents",
    "sley2.release-candidate-manifest.v1",
    "## 3. Conformance subset",
    "## 4. Canonical demo (source-independence proof)",
    "conformance/release-demo/v1/demo.json",
    "## 5. Forbidden content",
    "## 6. Reproducibility",
    "## 7. Evidence and blockers",
    "canonical detached linked worktree",
    "candidate-attestation-mismatch",
    "## 10. Explicit exclusions",
    "## 15. Artifact-content evidence",
    "sley2.candidate-content-checks.v1",
    "expected_artifact_members",
)
ADR_MARKERS = (
    "# ADR-0038: release candidate mechanics without a release",
    "1. **Mechanics now, gate later.**",
    "2. **Deterministic archive.**",
    "3. **No local path.**",
    "4. **The demo is the endpoint.**",
    "5. **Inventory reused.**",
    "6. **Codes.**",
    "7. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md`", "ADR-0038")
SCRIPT_MARKERS = (
    "class PackageErrorCode(IntEnum)",
    '"sley2.release-candidate-manifest.v1"',
    "--remap-path-prefix",
    "def remap_flags(",
    "ARTIFACT_INPUT_PATHS",
    "--allow-dirty",
    "working_tree_clean",
    "def deterministic_tar(",
    "def build_manifest(",
    "def scan_forbidden_content(",
    "def compare_artifacts(",
    "def run_conformance_subset(",
    "def run_demo(",
    "def build_candidate(",
    "sley-2.0.0-linux-x86_64",
)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def load_repro():
    """The S20-730 builder: the owner of the attestation admissibility rule."""
    spec = importlib.util.spec_from_file_location(
        "build_reproducibility_report", ROOT / "scripts/build_reproducibility_report.py"
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def gate_stays_closed(gate: str) -> bool:
    completed = subprocess.run(
        [sys.executable, "scripts/gate_status.py", gate],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 2:
        return False
    try:
        return json.loads(completed.stdout).get("result") == "NOT_IMPLEMENTED"
    except json.JSONDecodeError:
        return False


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2))
        return 1

    spec = read(SPEC)
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in spec:
            problems.append(f"spec-code:{symbol}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if "72000 through 72007" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")
    for gate in ("v2", "release-check"):
        if not gate_stays_closed(gate):
            problems.append(f"gate-not-fail-closed:{gate}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("release_candidate_packaging")
    if not isinstance(section, dict):
        problems.append("machine-summary:release_candidate_packaging missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md",
        "adr": "docs/adr/ADR-0038-release-candidate-packaging-boundary.md",
        "artifact_name": "sley-2.0.0-linux-x86_64.tar.gz",
        "manifest_contract": "sley2.release-candidate-manifest.v1",
        "new_stable_error_codes": len(CODES),
        "release_check_gate": "FAIL_CLOSED_NOT_IMPLEMENTED",
        "ga_claimed": False,
        "publication_authorized": False,
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")
    for key, expected in (
        ("contract_revision", 5),
        ("candidate_content_report", "evidence/release/candidate-content-checks.json"),
        ("candidate_content_checker", "scripts/build_candidate_content_report.py"),
    ):
        if section.get(key) != expected:
            problems.append(f"machine-summary:{key}")
    if "revision 5 (2026-09-15)" not in spec:
        problems.append("spec-revision")
    content_script = ROOT / "scripts/build_candidate_content_report.py"
    if not content_script.exists() or "sley2.candidate-content-checks.v1" not in read(content_script):
        problems.append("candidate-content:missing-owner-script")
    artifact = summary.get("artifact", {})
    if any(artifact.get(key) is not None for key in ("path", "sha256", "size_bytes", "reproducibility")):
        problems.append("machine-summary:artifact-not-null")
    # The register's candidate identity must name the tracked
    # reproducibility attestation: without this cross-check the section can
    # name any candidate while the checker stays green. Admissibility is
    # the S20-730 builder's predicate (clean, REPRODUCIBLE, non-empty
    # toolchain strings, contract shape), imported rather than restated, and
    # the quality fields the register hand-maintains must agree with it.
    repro_path = ROOT / "evidence/release/reproducibility-report.json"
    if status in IMPLEMENTATION_STATUSES:
        if not repro_path.exists():
            problems.append("machine-summary:candidate-attestation-unbound")
        else:
            repro = json.loads(read(repro_path))
            attested = {
                (
                    attestation.get("commit"),
                    attestation.get("artifact_sha256"),
                    attestation.get("manifest_digest"),
                    attestation.get("artifact_size_bytes"),
                    attestation.get("member_count"),
                    attestation.get("working_tree_clean"),
                    attestation.get("reproducibility"),
                    (attestation.get("toolchain") or {}).get("cargo"),
                    (attestation.get("toolchain") or {}).get("rustc"),
                )
                for attestation in load_repro().admissible_attestations(repro)
            }
            candidate_toolchain = section.get("candidate_toolchain") or {}
            if (
                section.get("candidate_commit"),
                section.get("candidate_artifact_sha256"),
                section.get("candidate_manifest_digest"),
                section.get("candidate_artifact_size_bytes"),
                section.get("candidate_member_count"),
                section.get("candidate_working_tree_clean"),
                section.get("candidate_reproducibility"),
                candidate_toolchain.get("cargo"),
                candidate_toolchain.get("rustc"),
            ) not in attested:
                problems.append("machine-summary:candidate-attestation-mismatch")

    present = []
    if SCRIPT.exists():
        present.append("scripts/build_release_candidate.py")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        script = read(SCRIPT) if SCRIPT.exists() else ""
        for marker in SCRIPT_MARKERS:
            if marker not in script:
                problems.append(f"script-marker:{marker}")
        for _, symbol in CODES:
            if symbol not in script:
                problems.append(f"script-code:{symbol}")
        if not DEMO.exists():
            problems.append("demo:missing")
        if not FIXTURE.exists():
            problems.append("fixture:missing")
        if not TESTS.exists():
            problems.append("tests:missing")
        else:
            completed = subprocess.run(
                [sys.executable, "-m", "unittest", "discover", "-s", "bench/release/tests", "-t", "."],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            if completed.returncode != 0:
                problems.append("tests:failed:" + completed.stderr.strip().splitlines()[-1][:200])
        # offline_tests names the test-method count in test_packaging.py;
        # the field drifted silently before, so the checker pins it.
        packaging_tests = (ROOT / "bench/release/tests/test_packaging.py").read_text(
            encoding="utf-8"
        )
        if section.get("offline_tests") != len(
            re.findall(r"    def (test_\w+)", packaging_tests)
        ):
            problems.append("machine-summary:offline-tests-drift")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-720-release-candidate-packaging-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
        "implementation_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

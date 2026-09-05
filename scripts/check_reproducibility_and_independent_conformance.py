#!/usr/bin/env python3
"""Check the S20-730 reproducibility and independent conformance contract and its stage."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md"
ADR = ROOT / "docs/adr/ADR-0040-reproducibility-and-independent-conformance-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
REPRO_SCRIPT = ROOT / "scripts/build_reproducibility_report.py"
CONFORMANCE_SCRIPT = ROOT / "scripts/build_independent_conformance_report.py"
TESTS = ROOT / "bench/release/tests/test_reproducibility.py"
REPRO_REPORT = ROOT / "evidence/release/reproducibility-report.json"
CONFORMANCE_REPORT = ROOT / "evidence/conformance/independent-conformance-report.json"

DRAFT_STATUS = "S20_730_CONTRACT_DRAFT_REVIEW_PENDING"
IN_PROGRESS_STATUS = "S20_730_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_730_MECHANICS_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_730_COMPLETE"
IMPLEMENTATION_STATUSES = (IN_PROGRESS_STATUS, REVIEW_PENDING_STATUS, COMPLETE_STATUS)

CODES = (
    (73000, "REPRO_EVIDENCE_MISSING"),
    (73001, "REPRO_EVIDENCE_INVALID"),
    (73002, "REPRO_ATTESTATION_INVALID"),
    (73003, "REPRO_ATTESTATION_CONFLICT"),
    (73004, "CONFORMANCE_FIXTURE_UNREADABLE"),
    (73005, "CONFORMANCE_SUMS_MISMATCH"),
    (73006, "CONFORMANCE_ORACLE_DRIFT"),
    (73007, "CONFORMANCE_REPORT_DRIFT"),
)
SPEC_MARKERS = (
    "# Reproducibility and Independent Conformance v1",
    "Status: S20-730 contract draft",
    "## 1. Reproducibility attestations",
    "sley2.reproducibility-attestation.v1",
    "## 2. Reproducibility report",
    "sley2.reproducibility-report.v1",
    "MULTI_HOST_REPRODUCIBLE",
    "GATED_OPERATOR_LANE",
    "## 3. Independent conformance report",
    "sley2.independent-conformance-report.v1",
    "## 4. Oracle independence",
    "## 5. Coverage classes",
    "## 6. Evidence files",
    "## 7. Codes",
    "## 8. Staging",
    "## 9. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0040: reproducibility attestations and independent conformance as derived evidence",
    "1. **Attestations, not assertions.**",
    "2. **Conflicts fail closed.**",
    "3. **Coverage is declared.**",
    "4. **Derived from the commit.**",
    "5. **No identity leakage.**",
    "6. **Codes.**",
    "7. **Staging.**",
)
WORK_PACKAGE_MARKERS = (
    "`docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`",
    "ADR-0040",
)
REPRO_MARKERS = (
    "class ReproErrorCode(IntEnum)",
    '"sley2.reproducibility-attestation.v1"',
    "def local_attestation(",
    "def validate_attestation(",
    "def build_report(",
    "def verify_report(",
    "def carried_attestations(",
    "REQUIRED_HOSTS = 2",
)
CONFORMANCE_MARKERS = (
    "class ConformanceErrorCode(IntEnum)",
    '"sley2.independent-conformance-report.v1"',
    "def conformance_recipe(",
    "def family_record(",
    "def oracle_independence(",
    "COVERAGE: dict[str, str | None]",
    "DEPTH: dict[str, str]",
    "coverage_depths",
)
# Neither report may carry a host name, a user name, a path outside the tree,
# or a time (contract section 9).
FORBIDDEN_REPORT_MARKERS = ("/home/", "greyforge", "timestamp", "generated_at")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


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


def run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, *argv], cwd=ROOT, check=False, capture_output=True, text=True
    )


def load_module(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"scripts/{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def git_text(args: list[str]) -> str | None:
    """Stdout of a read-only git command, or None when history is unavailable."""
    completed = subprocess.run(
        ["git", *args], cwd=ROOT, check=False, capture_output=True, text=True
    )
    if completed.returncode != 0:
        return None
    return completed.stdout


def history_problems(report: dict, surface: tuple[str, ...]) -> list[str]:
    """Bind every attested commit to the filing history (contract section 2).

    Ancestry ties the attestation to this tree; the artifact-surface diff
    keeps a changed tree from presenting an old candidate as current. Both
    run on git history and the working tree, so they need no gitignored
    local evidence and stay hermetic on a clean checkout.
    """
    problems: list[str] = []
    head = git_text(["rev-parse", "HEAD"])
    if head is None:
        return ["reproducibility-report:history-unavailable"]
    head = head.strip()
    commits = sorted(
        {
            attestation.get("commit", "")
            for attestation in report.get("attestations", [])
            if isinstance(attestation, dict)
        }
    )
    for commit in commits:
        if git_text(["merge-base", "--is-ancestor", commit, head]) is None:
            problems.append(
                f"reproducibility-report:attestation-not-in-history:{commit[:12]}"
            )
            continue
        changed = git_text(["diff", "--name-only", commit, head, "--", *surface])
        if changed is None:
            problems.append("reproducibility-report:history-unavailable")
            continue
        if changed.strip():
            names = sorted(changed.split())
            problems.append(
                f"reproducibility-report:stale:{commit[:12]}:"
                f"{len(names)}-surface-files-changed:{','.join(names[:8])}"
            )
    worktree = git_text(["status", "--porcelain", "--", *surface])
    if worktree is None:
        problems.append("reproducibility-report:history-unavailable")
    elif worktree.strip():
        names = sorted(line[3:] for line in worktree.splitlines() if line.strip())
        problems.append(
            f"reproducibility-report:uncommitted-surface-changes:"
            f"{len(names)}:{','.join(names[:8])}"
        )
    return problems


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2, sort_keys=True))
        return 1

    spec = read(SPEC)
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-missing:{marker}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-missing:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-missing:{marker}")
    codes = read(ERROR_CODES)
    if "S20-730 reserves numeric codes 73000 through 73007" not in codes:
        problems.append("error-codes-missing:reservation")
    for number, name in CODES:
        if name not in codes:
            problems.append(f"error-codes-missing:{number}:{name}")
        if name not in spec:
            problems.append(f"spec-missing-code:{name}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("reproducibility_and_independent_conformance")
    if not isinstance(section, dict):
        print(
            json.dumps(
                {"problems": ["machine-summary:section"], "result": "FAIL"},
                indent=2,
                sort_keys=True,
            )
        )
        return 1
    status = section.get("status")
    if status not in (DRAFT_STATUS, *IMPLEMENTATION_STATUSES):
        problems.append("machine-summary:status")
    for key, value in (
        ("contract", "docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md"),
        ("adr", "docs/adr/ADR-0040-reproducibility-and-independent-conformance-boundary.md"),
        ("checker", "scripts/check_reproducibility_and_independent_conformance.py"),
        ("reproducibility_report", "evidence/release/reproducibility-report.json"),
        (
            "independent_conformance_report",
            "evidence/conformance/independent-conformance-report.json",
        ),
        ("new_stable_error_codes", 8),
        ("new_error_code_range", "73000 through 73007"),
        ("ga_claimed", False),
        ("publication_authorized", False),
        ("implementation_complete", status == COMPLETE_STATUS),
    ):
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status == COMPLETE_STATUS:
        for review in (
            "ariadne_contract_review",
            "nabu_architecture_review",
            "vulcan_surface_review",
        ):
            if section.get(review) != "PASS":
                problems.append(f"machine-summary:{review}")

    if status in IMPLEMENTATION_STATUSES:
        for path, markers in (
            (REPRO_SCRIPT, REPRO_MARKERS),
            (CONFORMANCE_SCRIPT, CONFORMANCE_MARKERS),
        ):
            if not path.exists():
                problems.append(f"missing:{path.relative_to(ROOT)}")
                continue
            text = read(path)
            for marker in markers:
                if marker not in text:
                    problems.append(f"{path.name}-missing:{marker}")
        if not TESTS.exists():
            problems.append(f"missing:{TESTS.relative_to(ROOT)}")

        for path, contract, result_key, allowed in (
            (
                REPRO_REPORT,
                "sley2.reproducibility-report.v1",
                "result",
                ("SINGLE_HOST_REPRODUCIBLE", "MULTI_HOST_REPRODUCIBLE"),
            ),
            (
                CONFORMANCE_REPORT,
                "sley2.independent-conformance-report.v1",
                "result",
                ("INDEPENDENT_CONFORMANCE_PARTIAL", "INDEPENDENT_CONFORMANCE_COMPLETE"),
            ),
        ):
            if not path.exists():
                problems.append(f"missing:{path.relative_to(ROOT)}")
                continue
            text = read(path)
            for marker in FORBIDDEN_REPORT_MARKERS:
                if marker in text:
                    problems.append(f"{path.name}-forbidden:{marker}")
            report = json.loads(text)
            if report.get("contract") != contract:
                problems.append(f"{path.name}:contract")
            if report.get(result_key) not in allowed:
                problems.append(f"{path.name}:result")
            if report.get("work_package") != "S20-730":
                problems.append(f"{path.name}:work_package")

        if REPRO_REPORT.exists():
            report = json.loads(read(REPRO_REPORT))
            if not report.get("attestations"):
                problems.append("reproducibility-report:no-attestation")
            if report.get("ga_claimed") is not False:
                problems.append("reproducibility-report:ga_claimed")
            if report.get("publication_authorized") is not False:
                problems.append("reproducibility-report:publication_authorized")
            repro = load_module("build_reproducibility_report")
            for integrity in repro.verify_report(report):
                problems.append(f"reproducibility-report:integrity:{integrity}")
            try:
                candidate = load_module("build_release_candidate")
                surface = tuple(candidate.ARTIFACT_INPUT_PATHS)
            except Exception:
                candidate = None
                problems.append("reproducibility-report:artifact-surface-unknown")
                surface = ()
            if surface:
                problems.extend(history_problems(report, surface))
            attested_toolchains = {
                (
                    attestation.get("toolchain", {}).get("cargo"),
                    attestation.get("toolchain", {}).get("rustc"),
                )
                for attestation in report.get("attestations", [])
                if isinstance(attestation, dict)
            }
            try:
                if candidate is None:
                    candidate = load_module("build_release_candidate")
                current_toolchain = candidate.toolchain_versions()
                current = (current_toolchain.get("cargo"), current_toolchain.get("rustc"))
            except Exception:
                current = None
                problems.append("reproducibility-report:toolchain-unavailable")
            if current is not None:
                for cargo, rustc in sorted(attested_toolchains):
                    if (cargo, rustc) != current:
                        problems.append(
                            "reproducibility-report:toolchain-changed:"
                            f"attested-{cargo}-plus-{rustc}"
                        )

        if CONFORMANCE_REPORT.exists():
            conformance = json.loads(read(CONFORMANCE_REPORT))
            depths: dict[str, list[str]] = {"semantic": [], "codec_and_identity": []}
            for family in conformance.get("fixtures", []):
                coverage = family.get("coverage", {})
                if coverage.get("kind") != "independent_oracle":
                    continue
                if coverage.get("depth") not in depths:
                    problems.append(
                        f"independent-conformance-report:undeclared-depth:{family.get('directory')}"
                    )
                    continue
                depths[coverage["depth"]].append(family.get("directory"))
            if conformance.get("coverage_depths") != {
                depth: sorted(directories) for depth, directories in depths.items()
            }:
                problems.append("independent-conformance-report:depth-rollup-mismatch")

        drift = run(["scripts/build_independent_conformance_report.py", "--check"])
        if drift.returncode != 0:
            problems.append("independent-conformance-report:drift")
        tests = run(["-m", "unittest", "discover", "-s", "bench/release/tests", "-t", "."])
        if tests.returncode != 0:
            problems.append("release-tests:fail")

    for gate in ("v2", "release-check"):
        if not gate_stays_closed(gate):
            problems.append(f"gate-open:{gate}")

    result = {
        "codes": [name for _, name in CODES],
        "contract": "s20-730-reproducibility-and-independent-conformance-v1",
        "implementation_complete": status == COMPLETE_STATUS,
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

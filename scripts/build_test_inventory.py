#!/usr/bin/env python3
"""S20-750 source: the tracked inventory of tests and conformance vectors.

Counts, from tracked sources only, the Rust unit tests per crate, the ignored
fixture-refresh emitters, the persistent fuzz targets, the Python test
functions, the conformance vectors per fixture family, and the property tests
per harness, so the completion report's "property-test counts" item is
evidenced rather than estimated. A property-test count of zero is a counted
fact, not an absence of evidence: the scan names the manifests, lockfile, and
sources it searched. It runs nothing: the counts describe the corpus, not a
test run.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
FUZZ_TARGETS = ROOT / "fuzz/targets"
BENCH = ROOT / "bench"
ORACLE_TESTS = ROOT / "oracle/scb1/tests"
CONFORMANCE = ROOT / "conformance"
INVENTORY = ROOT / "evidence/validation/test-inventory.json"
CONTRACT = "sley2.test-inventory.v1"
TEST_ATTRIBUTE = re.compile(r"^\s*#\[test\]\s*$", re.M)
IGNORE_ATTRIBUTE = re.compile(r"^\s*#\[ignore", re.M)
PYTHON_TEST = re.compile(r"^\s*def (test_[A-Za-z0-9_]+)", re.M)
# Property-test harnesses: a use site is a macro invocation or a qualified
# path in Rust, a @given decorator or hypothesis import in Python, and a
# harness dependency is a manifest dependency entry naming the harness.
RUST_PROPERTY_USE = re.compile(r"\b(?:proptest!|quickcheck!|proptest::|quickcheck::)")
PYTHON_PROPERTY_USE = re.compile(r"@given\b|from hypothesis\b|import hypothesis\b")
MANIFEST_HARNESS = re.compile(r"^\s*(?:proptest|quickcheck|hypothesis)\b", re.M)


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def rust_counts() -> tuple[list[dict], int, int]:
    crates: list[dict] = []
    for crate in sorted(path for path in CRATES.iterdir() if path.is_dir()):
        tests = 0
        ignored = 0
        for source in sorted(crate.rglob("*.rs")):
            text = source.read_text(encoding="utf-8")
            tests += len(TEST_ATTRIBUTE.findall(text))
            ignored += len(IGNORE_ATTRIBUTE.findall(text))
        if tests or ignored:
            crates.append({"crate": crate.name, "tests": tests, "ignored": ignored})
    return crates, sum(item["tests"] for item in crates), sum(item["ignored"] for item in crates)


def python_counts() -> list[dict]:
    modules: list[dict] = []
    directories = sorted(
        {path.parent for path in BENCH.rglob("tests/test_*.py")} | {ORACLE_TESTS}
    )
    for directory in directories:
        if not directory.is_dir():
            continue
        tests = 0
        files = 0
        for source in sorted(directory.glob("test_*.py")):
            tests += len(PYTHON_TEST.findall(source.read_text(encoding="utf-8")))
            files += 1
        if files:
            modules.append(
                {
                    "directory": str(directory.relative_to(ROOT)),
                    "files": files,
                    "tests": tests,
                }
            )
    return modules


def conformance_counts() -> list[dict]:
    families: list[dict] = []
    for family in sorted(path for path in CONFORMANCE.iterdir() if path.is_dir()):
        for version in sorted(path for path in family.iterdir() if path.is_dir()):
            vectors = 0
            rejections = 0
            for source in sorted(version.glob("*.json")):
                value = json.loads(source.read_text(encoding="utf-8"))
                if not isinstance(value, dict):
                    continue
                for key, item in value.items():
                    if isinstance(item, list):
                        if key in ("mutations",) or "reject" in source.stem:
                            rejections += len(item)
                        elif key.endswith("vectors") or key in ("methods", "vectors"):
                            vectors += len(item)
                    elif (
                        isinstance(item, dict)
                        and key == "cases"
                        and "inputs" not in source.stem
                    ):
                        # Identifier-keyed case maps (entity-read accepted):
                        # each case is one vector. Authored input matrices
                        # are not vectors.
                        vectors += len(item)
            families.append(
                {
                    "family": family.name,
                    "version": version.name,
                    "vectors": vectors,
                    "rejections": rejections,
                }
            )
    return families


def property_counts() -> dict:
    """Counted property tests per harness, from tracked sources only.

    The scan covers every crate manifest plus the workspace manifest and
    lockfile for harness dependencies, every Rust source for harness use
    sites, and every counted Python test module for hypothesis use sites.
    A zero is a counted zero: the detail names what was searched.
    """
    manifests = CRATES_manifests()
    rust_uses = 0
    for source in sorted(CRATES.rglob("*.rs")):
        rust_uses += len(RUST_PROPERTY_USE.findall(source.read_text(encoding="utf-8")))
    python_uses = 0
    for directory in sorted(
        {path.parent for path in BENCH.rglob("tests/test_*.py")} | {ORACLE_TESTS}
    ):
        if not directory.is_dir():
            continue
        for source in sorted(directory.glob("test_*.py")):
            python_uses += len(PYTHON_PROPERTY_USE.findall(source.read_text(encoding="utf-8")))
    harness_deps = sorted(
        {
            match.group(0).strip()
            for manifest in manifests
            for match in MANIFEST_HARNESS.finditer(manifest.read_text(encoding="utf-8"))
        }
    )
    return {
        "rust_use_sites": rust_uses,
        "python_use_sites": python_uses,
        "manifest_harness_deps": harness_deps,
        "scanned_manifests": len(manifests),
    }


def CRATES_manifests() -> list[Path]:
    manifests = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    manifests.extend(sorted(CRATES.glob("*/Cargo.toml")))
    return [path for path in manifests if path.is_file()]


def build_inventory() -> dict:
    crates, rust_tests, rust_ignored = rust_counts()
    modules = python_counts()
    families = conformance_counts()
    fuzz_targets = sorted(path.stem for path in FUZZ_TARGETS.glob("*.rs"))
    property_detail = property_counts()
    inventory = {
        "contract": CONTRACT,
        "work_package": "S20-750",
        "source": "tracked sources only; no test was executed to produce these counts",
        "rust_crates": crates,
        "rust_unit_tests": rust_tests,
        "rust_ignored_emitters": rust_ignored,
        "persistent_fuzz_targets": fuzz_targets,
        "persistent_fuzz_target_count": len(fuzz_targets),
        "python_test_modules": modules,
        "python_tests": sum(item["tests"] for item in modules),
        "conformance_families": families,
        "conformance_vectors": sum(item["vectors"] for item in families),
        "conformance_rejections": sum(item["rejections"] for item in families),
        "property_test_detail": property_detail,
        "property_tests": property_detail["rust_use_sites"] + property_detail["python_use_sites"],
    }
    inventory["inventory_digest"] = digest_of(inventory)
    return inventory


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    inventory = build_inventory()
    text = canonical(inventory)
    summary = {
        "rust_unit_tests": inventory["rust_unit_tests"],
        "python_tests": inventory["python_tests"],
        "persistent_fuzz_targets": inventory["persistent_fuzz_target_count"],
        "conformance_vectors": inventory["conformance_vectors"],
    }
    if args.check:
        current = INVENTORY.read_text(encoding="utf-8") if INVENTORY.exists() else None
        if current != text:
            print(
                canonical(
                    {
                        "mode": "check",
                        "result": "FAIL",
                        "detail": "the tracked test inventory differs from the derived inventory",
                    }
                ),
                end="",
            )
            return 1
        print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
        return 0
    INVENTORY.parent.mkdir(parents=True, exist_ok=True)
    INVENTORY.write_text(text, encoding="utf-8")
    print(canonical({"mode": "write", "result": "PASS", **summary}), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())

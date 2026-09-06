#!/usr/bin/env python3
"""S20-730 independent conformance report: coverage of every fixture family.

Derives `evidence/conformance/independent-conformance-report.json` from
tracked files only: every `conformance/<family>/v1` directory, its file
digests, its declared coverage (an independent oracle command of the
`make conformance` recipe at a declared semantic or codec-and-identity
depth, or native-only), and the S20-130 oracle independence scan. Nothing
here runs cargo or the oracle.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from enum import IntEnum
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONFORMANCE = ROOT / "conformance"
ORACLE = ROOT / "oracle/scb1"
MAKEFILE = ROOT / "Makefile"
REPORT = ROOT / "evidence/conformance/independent-conformance-report.json"
REPORT_CONTRACT = "sley2.independent-conformance-report.v1"
FORBIDDEN_MARKERS = (
    "subprocess",
    "cargo",
    "sley-id",
    "sley_id",
    "sley-mutate",
    "sley_mutate",
    "crates/",
    "value_generated.rs",
    "target/",
    # Linking the Rust library would be an implementation dependency that no
    # string above catches.
    "ctypes",
    "cffi",
    "os.system",
    "popen",
)

ORACLE_RUNNER = "uv run --project oracle/scb1 --frozen"
# Every fixture family maps to the independent oracle command that checks it
# (contract section 3) or to a native-only note. Adding a family without a
# mapping is CONFORMANCE_ORACLE_DRIFT.
COVERAGE: dict[str, str | None] = {
    "bootstrap-capability": "python3 scripts/check_bootstrap_capability.py",
    "bootstrap-profile": "python3 scripts/check_bootstrap_profile_1.py",
    "candidate-result": f"{ORACLE_RUNNER} sley2-scb1-oracle check-candidate-result",
    "complete-entity-impact": "python3 scripts/check_complete_entity_impact_vector.py",
    "complete-root-index-snapshot": f"{ORACLE_RUNNER} python scripts/check_complete_root_index_snapshot_vector.py",
    "context-capsule": f"{ORACLE_RUNNER} python scripts/check_context_capsule_vector.py",
    "host-abi": "python3 scripts/check_host_abi_v1.py",
    "merge": f"{ORACLE_RUNNER} python scripts/check_merge_vector.py",
    "mutation-candidate": f"{ORACLE_RUNNER} sley2-scb1-oracle check-mutation-candidate",
    "mutation-value": f"{ORACLE_RUNNER} sley2-scb1-oracle check-mutation-value",
    "release-demo": f"{ORACLE_RUNNER} python scripts/check_release_demo_vector.py",
    "repository-exchange": f"{ORACLE_RUNNER} python scripts/check_repository_exchange_vector.py",
    "repository-pack": f"{ORACLE_RUNNER} python scripts/check_repository_pack_vector.py",
    "root-backed-query": f"{ORACLE_RUNNER} python scripts/check_root_backed_query_vector.py",
    "scb1": f"{ORACLE_RUNNER} sley2-scb1-oracle check",
    "schema-epoch": f"{ORACLE_RUNNER} python scripts/check_schema_epoch_vector.py",
    "semantic-comparison": f"{ORACLE_RUNNER} python scripts/check_semantic_comparison_vector.py",
    "smp1": f"{ORACLE_RUNNER} python scripts/check_smp1_vector.py",
    "smp1-json-bridge": f"{ORACLE_RUNNER} python scripts/check_smp1_json_bridge_vector.py",
    "state-root": f"{ORACLE_RUNNER} python scripts/check_state_root_vector.py",
    "transaction-receipt": f"{ORACLE_RUNNER} sley2-scb1-oracle check-transaction-receipt",
    "vm-extended": f"{ORACLE_RUNNER} sley2-scb1-oracle check-vm-extended --accepted conformance/vm-extended/v1/accepted.json --rejected conformance/vm-extended/v1/rejected.json",
}
# Coverage depth per family (contract section 5): "semantic" when the checker
# recomputes an outcome or judgment from frozen inputs with independent logic
# (deltas, edge closures, query results, merge judgments); "codec_and_identity"
# when it decodes containers and re-derives records, identities, keys, or
# digest trees without judging semantics. The assignments follow what each
# checker recomputes: the merge checker applies the merge judgment, the
# semantic-comparison checker re-derives change classes and deltas, the
# complete-entity-impact checker re-derives the edge set and its closure, and
# the root-backed-query checker re-derives result pages, accounting, and
# failure precedence. Every other checker reconstructs encodings, identities,
# or digest trees and compares them against the recorded fixtures.
DEPTH: dict[str, str] = {
    "bootstrap-capability": "codec_and_identity",
    "bootstrap-profile": "codec_and_identity",
    "candidate-result": "codec_and_identity",
    "complete-entity-impact": "semantic",
    "complete-root-index-snapshot": "codec_and_identity",
    "context-capsule": "codec_and_identity",
    "host-abi": "codec_and_identity",
    "merge": "semantic",
    "mutation-candidate": "codec_and_identity",
    "mutation-value": "codec_and_identity",
    "release-demo": "codec_and_identity",
    "repository-exchange": "codec_and_identity",
    "repository-pack": "codec_and_identity",
    "root-backed-query": "semantic",
    "scb1": "codec_and_identity",
    "schema-epoch": "codec_and_identity",
    "semantic-comparison": "semantic",
    "smp1": "codec_and_identity",
    "smp1-json-bridge": "codec_and_identity",
    "state-root": "codec_and_identity",
    "transaction-receipt": "codec_and_identity",
    "vm-extended": "codec_and_identity",
}
COVERAGE_DEPTHS = ("semantic", "codec_and_identity")
# Every family now has an independent checker; the mapping stays so a future
# family can be declared native-only with its reason.
NATIVE_ONLY_NOTES: dict[str, str] = {}


class ConformanceErrorCode(IntEnum):
    """S20-730 independent conformance failures (contract section 7)."""

    FIXTURE_UNREADABLE = 73004
    SUMS_MISMATCH = 73005
    ORACLE_DRIFT = 73006
    REPORT_DRIFT = 73007


class ConformanceError(Exception):
    """One exact S20-730 failure."""

    def __init__(self, code: ConformanceErrorCode, detail: str) -> None:
        super().__init__(f"{code.name}: {detail}")
        self.code = code
        self.detail = detail


def display(path: Path) -> str:
    """The repository-relative path when the path is inside the tree."""
    return str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def conformance_recipe() -> str:
    """The body of the `conformance:` recipe of the Makefile."""
    lines = MAKEFILE.read_text(encoding="utf-8").split("\n")
    try:
        start = lines.index("conformance:")
    except ValueError as error:
        raise ConformanceError(
            ConformanceErrorCode.ORACLE_DRIFT, "the Makefile has no conformance target"
        ) from error
    body: list[str] = []
    for line in lines[start + 1 :]:
        if not line.startswith("\t"):
            break
        body.append(line.strip())
    return "\n".join(body)


def fixture_shape(value: object) -> dict:
    """The list-valued keys of one fixture, so vector counts are recorded."""
    if not isinstance(value, dict):
        return {"root": len(value) if isinstance(value, list) else 0}
    return {key: len(item) for key, item in sorted(value.items()) if isinstance(item, list)}


def read_sums(path: Path) -> dict[str, str]:
    entries: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").split("\n"):
        if not line.strip():
            continue
        parts = line.split()
        if len(parts) != 2:
            raise ConformanceError(
                ConformanceErrorCode.SUMS_MISMATCH, f"{display(path)}: malformed line"
            )
        entries[parts[1].lstrip("*")] = parts[0]
    return entries


def family_record(directory: Path, recipe: str) -> dict:
    """One fixture family: digests, declared coverage, and sums consistency."""
    name = directory.name
    if name not in COVERAGE:
        raise ConformanceError(
            ConformanceErrorCode.ORACLE_DRIFT,
            f"fixture family {name!r} declares no coverage; add it to COVERAGE",
        )
    version = directory / "v1"
    if not version.is_dir():
        raise ConformanceError(
            ConformanceErrorCode.FIXTURE_UNREADABLE, f"{name} has no v1 directory"
        )
    files: list[dict] = []
    shape: dict[str, dict] = {}
    contract: str | None = None
    claim: str | None = None
    for path in sorted(version.iterdir()):
        if not path.is_file():
            continue
        data = path.read_bytes()
        files.append(
            {
                "name": path.name,
                "sha256": hashlib.sha256(data).hexdigest(),
                "bytes": len(data),
            }
        )
        if path.suffix != ".json":
            continue
        try:
            value = json.loads(data.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ConformanceError(
                ConformanceErrorCode.FIXTURE_UNREADABLE,
                f"{display(path)}: {error}",
            ) from error
        shape[path.name] = fixture_shape(value)
        if isinstance(value, dict):
            contract = contract or value.get("contract")
            claim = claim or value.get("claim")
    if not files:
        raise ConformanceError(ConformanceErrorCode.FIXTURE_UNREADABLE, f"{name} has no fixtures")

    sums_path = version / "SHA256SUMS"
    sums_consistent: bool | None = None
    if not sums_path.exists():
        # A manifest is how an independent party verifies a corpus without
        # running its generator, so every corpus carries one.
        raise ConformanceError(
            ConformanceErrorCode.FIXTURE_UNREADABLE,
            f"{display(version)} has no SHA256SUMS manifest",
        )
    if sums_path.exists():
        declared = read_sums(sums_path)
        actual = {entry["name"]: entry["sha256"] for entry in files if entry["name"].endswith(".json")}
        if declared != actual:
            raise ConformanceError(
                ConformanceErrorCode.SUMS_MISMATCH,
                f"{display(sums_path)} does not match the fixture digests",
            )
        sums_consistent = True

    command = COVERAGE[name]
    if command is None:
        coverage = {"kind": "native_only", "note": NATIVE_ONLY_NOTES[name]}
    else:
        if command not in recipe:
            raise ConformanceError(
                ConformanceErrorCode.ORACLE_DRIFT,
                f"{name}: {command!r} is not in the make conformance recipe",
            )
        if DEPTH.get(name) not in COVERAGE_DEPTHS:
            raise ConformanceError(
                ConformanceErrorCode.ORACLE_DRIFT,
                f"{name}: declares no coverage depth; add it to DEPTH",
            )
        coverage = {
            "kind": "independent_oracle",
            "depth": DEPTH[name],
            # Name the runner that actually runs, not the package that runs
            # most of them: twelve oracles live under `scripts/`.
            "runner": (
                "oracle/scb1 (Python, S20-130 independent)"
                if "sley2-scb1-oracle" in command
                else "scripts/ (Python, S20-130 independent)"
            ),
            "command": command,
        }
    return {
        "directory": f"conformance/{name}/v1",
        "files": files,
        "sums_file": sums_path.exists(),
        "sums_consistent": sums_consistent,
        "contract": contract,
        "claim": claim,
        "shape": shape,
        "coverage": coverage,
    }


def oracle_independence() -> dict:
    """Re-applies the S20-130 forbidden marker scan over every oracle.

    Twelve of the nineteen independent checks live under `scripts/`, so a scan
    of the package alone would vouch for oracles it never read.
    """
    sources = sorted((ORACLE / "src").rglob("*.py")) + [
        ROOT / "scripts" / name
        for name in sorted(
            {
                match
                for command in COVERAGE.values()
                if command
                for match in re.findall(r"scripts/(check_[a-z0-9_]+\.py)", command)
            }
        )
    ]
    problems: list[str] = []
    for path in sources:
        text = path.read_text(encoding="utf-8")
        for marker in FORBIDDEN_MARKERS:
            if marker in text:
                problems.append(f"{path.relative_to(ROOT)} contains forbidden marker {marker!r}")
    if problems:
        raise ConformanceError(ConformanceErrorCode.ORACLE_DRIFT, "; ".join(problems))
    return {
        "python_sources": len(sources),
        "forbidden_markers": list(FORBIDDEN_MARKERS),
        "problems": problems,
    }


def build_report() -> dict:
    recipe = conformance_recipe()
    families = [
        family_record(directory, recipe)
        for directory in sorted(CONFORMANCE.iterdir())
        if directory.is_dir()
    ]
    native_only = [
        family["directory"] for family in families if family["coverage"]["kind"] == "native_only"
    ]
    depths: dict[str, list[str]] = {depth: [] for depth in COVERAGE_DEPTHS}
    for family in families:
        coverage = family["coverage"]
        if coverage["kind"] == "independent_oracle":
            depths[coverage["depth"]].append(family["directory"])
    report = {
        "contract": REPORT_CONTRACT,
        "work_package": "S20-730",
        "make_target": "conformance",
        "fixture_directories": len(families),
        "independently_checked": len(families) - len(native_only),
        "native_only": native_only,
        "coverage_depths": {depth: sorted(directories) for depth, directories in depths.items()},
        "fixtures": families,
        "oracle_independence": oracle_independence(),
        "result": (
            "INDEPENDENT_CONFORMANCE_PARTIAL"
            if native_only
            else "INDEPENDENT_CONFORMANCE_COMPLETE"
        ),
    }
    report["report_digest"] = digest_of(report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=REPORT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        report = build_report()
        text = canonical(report)
        summary = {
            "fixture_directories": report["fixture_directories"],
            "independently_checked": report["independently_checked"],
            "native_only": report["native_only"],
            "result": report["result"],
        }
        if args.check:
            current = args.output.read_text(encoding="utf-8") if args.output.exists() else None
            if current != text:
                print(
                    canonical(
                        {
                            "mode": "check",
                            "result": "FAIL",
                            "code": int(ConformanceErrorCode.REPORT_DRIFT),
                            "name": ConformanceErrorCode.REPORT_DRIFT.name,
                            "detail": "the tracked report differs from the derived report",
                        }
                    ),
                    end="",
                )
                return 1
            print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
            return 0
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
        print(canonical({"mode": "write", "result": "PASS", **summary}), end="")
        return 0
    except (ConformanceError, OSError) as error:
        if isinstance(error, ConformanceError):
            payload = {
                "result": "FAIL",
                "code": int(error.code),
                "name": error.code.name,
                "detail": error.detail,
            }
        else:
            payload = {"result": "FAIL", "detail": str(error)}
        print(canonical(payload), end="")
        return 1


if __name__ == "__main__":
    sys.exit(main())

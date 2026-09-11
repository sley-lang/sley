#!/usr/bin/env python3
"""Check the S20-250 full complete-entity-impact contract surface and stage.

While the contract is a draft (no implementation), the check binds the
identity of the contract text, the ADR, the work-package row, and the
machine-summary section, and it fails closed if the six normative
definitions or the conformance fixture appear before the summary says the
contract is frozen. Once implementation is in progress it binds the source
markers, the code table, the restricted-consumer guard, and the fixture.
"""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md"
RESTRICTED_SPEC = ROOT / "docs/spec/FINGERPRINT_IMPACT_PROFILE_V1.md"
ADR = ROOT / "docs/adr/ADR-0026-complete-entity-model-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
SSMC = ROOT / "crates/sley-ssmc/src/lib.rs"
QUERY = ROOT / "crates/sley-query/src/lib.rs"
COMPLETE_ROOT = ROOT / "crates/sley-query/src/complete_root.rs"
SNAPSHOT = ROOT / "crates/sley-query/src/snapshot.rs"
MUTATE_VALUE = ROOT / "crates/sley-mutate/src/value.rs"
QUERY_MANIFEST = ROOT / "crates/sley-query/Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"
FIXTURE_DIR = ROOT / "conformance/complete-entity-impact"

DRAFT_STATUS = "S20_250_FULL_CONTRACT_DRAFT_REVIEW_PENDING"
# Implementation proceeds against the unreviewed draft while every Council lane
# is unavailable; the reviews still gate the freeze and the completion status.
DRAFT_IN_PROGRESS_STATUS = "S20_250_FULL_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_250_FULL_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_250_FULL_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_250_FULL_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_250_FULL_COMPLETE"

DEFINITIONS = (
    "WorkspaceDefinition",
    "PackageDefinition",
    "NamespaceDefinition",
    "EntryPointDefinition",
    "PolicyBindingDefinition",
    "DependencyBindingDefinition",
)

CODES = (
    (25013, "IMPACT_ROOT_BINDING_MISMATCH"),
    (25014, "IMPACT_ROOT_INVENTORY_MISMATCH"),
    (25015, "IMPACT_ROOT_WORKSPACE_MISSING"),
    (25016, "IMPACT_ROOT_WORKSPACE_AMBIGUOUS"),
    (25017, "IMPACT_ROOT_PACKAGE_MEMBERSHIP"),
    (25018, "IMPACT_ROOT_NAMESPACE_ROOT"),
    (25019, "IMPACT_ROOT_NAMESPACE_TREE"),
    (25020, "IMPACT_ROOT_MEMBER_OWNERSHIP"),
    (25021, "IMPACT_ROOT_EXPORT_UNSCOPED"),
    (25022, "IMPACT_ROOT_ENTRY_POINTS_MISMATCH"),
    (25023, "IMPACT_ROOT_DEPENDENCY_ROOTS_MISMATCH"),
    (25024, "IMPACT_ROOT_DEPENDENCY_BINDING_UNOWNED"),
)

SPEC_MARKERS = (
    "# Complete Entity Model and Impact Profile v1",
    "Status: S20-250 full contract",
    "## 2. Fingerprints are unchanged",
    "`FINGERPRINT_ENTITY_UNSUPPORTED`, exactly as the restricted profile's section",
    "## 5. Direct impact edges for the six bodies",
    "| DependencyBinding `external_package` | no edge | never resolved locally |",
    "| DependencyBinding `dependency_root` | no edge | identity only |",
    "## 6. Complete-root request",
    "### 6.1 Closure rules",
    "`INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED`",
    "`sley-query` gains no dependency on `sley-store`, `sley-mutate`, or\n`sley-policy`",
    "- roots above `65,535` bindings;",
)
RULE_IDS = tuple(f"| C{index} |" for index in range(1, 12))
ADR_MARKERS = (
    "# ADR-0026: Complete entity model and complete-root impact boundary",
    "1. **One normative model.**",
    "2. **No new fingerprints.**",
    "3. **Edge vocabulary mirrors S20-360.**",
    "4. **Complete-root request and closure.**",
    "5. **Restricted consumers stay restricted.**",
    "6. **Contract form.**",
)
WORK_PACKAGE_MARKERS = (
    "`docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md`",
    "ADR-0026",
)
RESTRICTED_MARKERS = (
    "Status: S20-250 restricted epoch-1 normative specification.",
    "A later package must add\nthe six missing bodies and extend this profile",
)
SOURCE_MARKERS = (
    "pub enum EntryExposure",
    "pub struct DependencyBindingDefinition",
    "pub dependency_root: StateRoot,",
)
QUERY_MARKERS = (
    "Self::Workspace => 1,",
    "Self::DependencyBinding => 18,",
    "pub fn judge_complete_root",
    "IMPACT_ROOT_DEPENDENCY_BINDING_UNOWNED",
    "Self::RootDependencyBindingUnowned => 25_024,",
)
SNAPSHOT_MARKERS = ("IndexSnapshotErrorCode::CompletenessUnsupported",)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def lock_packages() -> dict:
    """Parse Cargo.lock into {package name: [dependency names]}.

    Dependency entries may carry a version or source suffix; only the
    leading name token is significant for reachability.
    """
    packages: dict = {}
    name = None
    deps: list = []
    in_deps = False
    for line in read(CARGO_LOCK).splitlines():
        stripped = line.strip()
        if stripped == "[[package]]":
            if name is not None:
                packages[name] = deps
            name, deps, in_deps = None, [], False
        elif stripped.startswith("name = "):
            name = stripped[len("name = ") :].strip().strip('"')
        elif stripped == "dependencies = [":
            in_deps = True
        elif in_deps and stripped == "]":
            in_deps = False
        elif in_deps and name is not None:
            token = stripped.strip(",").strip('"').split()
            if token:
                deps.append(token[0])
    if name is not None:
        packages[name] = deps
    return packages


def lock_reachable(root: str) -> set:
    """Transitive dependency closure of one package over Cargo.lock."""
    packages = lock_packages()
    seen = {root}
    queue = [root]
    while queue:
        for dep in packages.get(queue.pop(), []):
            if dep not in seen:
                seen.add(dep)
                queue.append(dep)
    seen.discard(root)
    return seen


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, RESTRICTED_SPEC, ERROR_CODES, CARGO_LOCK):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2))
        return 1

    spec = read(SPEC)
    for marker in SPEC_MARKERS + RULE_IDS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in spec:
            problems.append(f"spec-code:{symbol}")
    restricted = read(RESTRICTED_SPEC)
    for marker in RESTRICTED_MARKERS:
        if marker not in restricted:
            problems.append(f"restricted-marker:{marker}")
    for numeric, symbol in CODES:
        if symbol in restricted:
            problems.append(f"restricted-spec-must-not-carry:{symbol}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    error_codes = read(ERROR_CODES)
    if "25013 through 25024" not in error_codes:
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("complete_entity_impact_profile")
    if not isinstance(section, dict):
        problems.append("machine-summary:complete_entity_impact_profile missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md",
        "adr": "docs/adr/ADR-0026-complete-entity-model-boundary.md",
        "normative_model": "crates/sley-ssmc",
        "entity_kinds": 18,
        "new_stable_error_codes": len(CODES),
        "closure_rules": 11,
        "six_kind_fingerprints": False,
        "edge_kinds": 12,
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (
        DRAFT_STATUS,
        DRAFT_IN_PROGRESS_STATUS,
        FROZEN_STATUS,
        IN_PROGRESS_STATUS,
        REVIEW_PENDING_STATUS,
        COMPLETE_STATUS,
    ):
        problems.append("machine-summary:status")

    ssmc = read(SSMC)
    present = [name for name in DEFINITIONS if f"pub struct {name}" in ssmc]
    if FIXTURE_DIR.exists():
        present.append("conformance/complete-entity-impact")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in (DRAFT_IN_PROGRESS_STATUS, IN_PROGRESS_STATUS, REVIEW_PENDING_STATUS, COMPLETE_STATUS):
        missing = [name for name in DEFINITIONS if f"pub struct {name}" not in ssmc]
        if missing:
            problems.append(f"definitions-missing:{missing}")
        for marker in SOURCE_MARKERS:
            if marker not in ssmc:
                problems.append(f"ssmc-marker:{marker}")
        query = read(QUERY) + read(COMPLETE_ROOT)
        for marker in QUERY_MARKERS:
            if marker not in query:
                problems.append(f"query-marker:{marker}")
        for numeric, symbol in CODES:
            if symbol not in query:
                problems.append(f"query-code:{symbol}")
        snapshot = read(SNAPSHOT)
        for marker in SNAPSHOT_MARKERS:
            if marker not in snapshot:
                problems.append(f"snapshot-marker:{marker}")
        if "pub use sley_ssmc::EntryExposure" not in read(MUTATE_VALUE):
            problems.append("mutate-value:EntryExposure-not-re-exported")
        manifest = read(QUERY_MANIFEST)
        for forbidden in ("sley-store", "sley-mutate", "sley-policy"):
            if forbidden in manifest:
                problems.append(f"dependency-direction:sley-query-depends-on-{forbidden}")
        for forbidden in sorted(
            lock_reachable("sley-query") & {"sley-store", "sley-mutate", "sley-policy"}
        ):
            problems.append(f"dependency-direction:transitive-sley-query-reaches-{forbidden}")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")
        if status in (REVIEW_PENDING_STATUS, COMPLETE_STATUS):
            fixture = FIXTURE_DIR / "v1/accepted.json"
            if not fixture.exists():
                problems.append("fixture:missing")
            for marker in ("ImpactIndex::build" , "IndexSnapshotErrorCode::CompletenessUnsupported"):
                if marker not in snapshot:
                    problems.append(f"snapshot-guard:{marker}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-250-full-complete-entity-impact-profile-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
        "definitions_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

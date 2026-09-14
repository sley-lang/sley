#!/usr/bin/env python3
"""Check the S20-300 full complete-root index snapshot contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md"
RESTRICTED_SPEC = ROOT / "docs/spec/INDEX_SNAPSHOT_PROFILE_V1.md"
ADR = ROOT / "docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
SNAPSHOT = ROOT / "crates/sley-query/src/snapshot.rs"
CACHE = ROOT / "crates/sley-repo/src/index_cache.rs"
ROOT_QUERY = ROOT / "crates/sley-repo/src/root_query.rs"
EXCHANGE = ROOT / "crates/sley-repo/src/exchange.rs"
FIXTURE_DIR = ROOT / "conformance/complete-root-index-snapshot"

SPEC_REVISION = 3

DRAFT_STATUS = "S20_300_FULL_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_300_FULL_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_300_FULL_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_300_FULL_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_300_FULL_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (30008, "INDEX_SNAPSHOT_ROOT_INCOMPLETE"),
    (30009, "INDEX_SNAPSHOT_ROOT_MISMATCH"),
    (30010, "INDEX_SNAPSHOT_IO"),
)
SPEC_MARKERS = (
    "# Complete-Root Index Snapshot Profile v1",
    "Status: S20-300 full contract",
    "IndexCompleteness = RestrictedModeledKinds4To15Only(1) | CompleteRoot(2)",
    "> A snapshot digest authenticates bytes, never semantic provenance.",
    "## 5. Repository index cache",
    "`<repository>/index/v1/`",
    "never read\nthe cache",
    "`verify_cached_snapshot(repository, revision)`",
    "## 9. Explicit exclusions",
)
RESTRICTED_MARKERS = (
    "Status: S20-300 restricted epoch-1 normative specification.",
    "Full S20-300 and root-backed S20-310 remain blocked",
)
ADR_MARKERS = (
    "# ADR-0029: Complete-root index snapshot and cache reuse boundary",
    "1. **One record, two arms.**",
    "2. **Provenance by build path.**",
    "3. **One reuse path.**",
    "4. **Derived and disposable.**",
    "5. **Codes.**",
    "6. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`", "ADR-0029")
SNAPSHOT_MARKERS = (
    "CompleteRoot",
    "pub fn build_complete_root_snapshot",
    "pub fn admit_complete_root_snapshot",
    "const COMPLETENESS_COMPLETE_ROOT: u32 = 2;",
    "Self::RootIo => 30_010,",
)
CACHE_MARKERS = (
    "pub fn complete_root_snapshot",
    "pub fn verify_cached_snapshot",
    'const INDEX_DIRECTORY: &str = "index";',
    ".idx.scb1",
    "RepositoryMaintenanceGuard",
    "create_new(true)",
)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, RESTRICTED_SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES):
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
    restricted = read(RESTRICTED_SPEC)
    for marker in RESTRICTED_MARKERS:
        if marker not in restricted:
            problems.append(f"restricted-marker:{marker}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if "30008 through 30010" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("complete_root_index_snapshot")
    if not isinstance(section, dict):
        problems.append("machine-summary:complete_root_index_snapshot missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md",
        "adr": "docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md",
        "completeness_arm": 2,
        "entity_kinds": 18,
        "new_stable_error_codes": len(CODES),
        "cache_consumers": "READ_ONLY_DERIVED_QUERY_SURFACES_ONLY",
        "contract_revision": SPEC_REVISION,
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    snapshot = read(SNAPSHOT)
    present = []
    if "CompleteRoot" in snapshot:
        present.append("snapshot:CompleteRoot")
    if CACHE.exists():
        present.append("crates/sley-repo/src/index_cache.rs")
    if FIXTURE_DIR.exists():
        present.append("conformance/complete-root-index-snapshot")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        for marker in SNAPSHOT_MARKERS:
            if marker not in snapshot:
                problems.append(f"snapshot-marker:{marker}")
        for numeric, symbol in CODES:
            if symbol not in snapshot:
                problems.append(f"snapshot-code:{symbol}")
        cache = read(CACHE) if CACHE.exists() else ""
        for marker in CACHE_MARKERS:
            if marker not in cache:
                problems.append(f"cache-marker:{marker}")
        # The reuse path is allowlisted: only the designated transient-read
        # surface may take cache hits, and exported capsules build fresh.
        # A new caller fails this gate until it is deliberately listed.
        allowed_callers = {"crates/sley-repo/src/root_query.rs"}
        call_pattern = re.compile(r"(?<!build_)(?<!decode_)(?<!admit_)complete_root_snapshot\(")
        for path in sorted((ROOT / "crates").rglob("*.rs")):
            if "tests" in path.parts or path.name in ("index_cache.rs",):
                continue
            text = path.read_text(encoding="utf-8")
            if call_pattern.search(text) and "fn complete_root_snapshot" not in text:
                if str(path.relative_to(ROOT)) not in allowed_callers:
                    problems.append(f"cache-caller:{path.relative_to(ROOT)}")
        root_query = read(ROOT_QUERY) if ROOT_QUERY.exists() else ""
        if "run_root_query_fresh(revision" not in root_query:
            problems.append("capsule:not-fresh-only")
        # An incomplete clone may carry the cache directory, and the import
        # must remove it rather than adopt a record it cannot prove belongs to
        # the exchange (contract section 5). Presence of the allowlist entry is
        # not the property worth gating: the purge is.
        exchange = read(EXCHANGE)
        if "INDEX_DIRECTORY" not in exchange:
            problems.append("exchange-layout:index-directory-not-allowlisted")
        if "fn purge_index_cache" not in exchange:
            problems.append("exchange-layout:index-cache-not-purged-on-import")
        if "purge_index_cache(target)?" not in exchange:
            problems.append("exchange-layout:index-cache-purge-not-called")
        if "an_incomplete_clone_resumes_without_adopting_the_index_cache" not in exchange:
            problems.append("exchange-layout:index-cache-purge-untested")
        if status in (REVIEW_PENDING_STATUS, COMPLETE_STATUS):
            if not (FIXTURE_DIR / "v1/accepted.json").exists():
                problems.append("fixture:missing")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    # The revision is anchored to the Status header (not the first prose
    # occurrence) and pinned: a stale pin fails the moment the contract moves.
    own = re.search(r"^Status: S20-300 full contract draft, revision (\d+)", spec, flags=re.M)
    if own is None or int(own.group(1)) != SPEC_REVISION:
        problems.append("spec-revision")
    revision = own.group(1) if own else None
    result = {
        "contract": "s20-300-full-complete-root-index-snapshot-profile-v1",
        "status": status,
        "revision": int(revision) if revision else None,
        "implementation_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

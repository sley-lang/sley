#!/usr/bin/env python3
"""Check the S20-520 merge contract surface and its stage.

While the contract is a draft (no implementation), the check binds the frozen
identity of the contract text, the ADR, the work-package row, and the
machine-summary section, recomputes both frozen hashes from the preimage
texts, and fails closed if implementation surfaces appear before the summary
allows implementation.
"""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/MERGE_V1.md"
ADR = ROOT / "docs/adr/ADR-0028-merge-composition-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
IDENTIFIERS = ROOT / "docs/spec/IDENTIFIERS_V1.md"
IDENTIFIER_SOURCE = ROOT / "crates/sley-id/src/lib.rs"
SOURCE = ROOT / "crates/sley-repo/src/merge.rs"
FIXTURE_DIR = ROOT / "conformance/merge"
IMPLEMENTATION_SURFACES = (SOURCE, FIXTURE_DIR)

FIELD_SCHEMA_HASH = "008e0676f911dc82b605286624c855a21b2e9e9d397358bf0ce7f196f55c2c8b"
DECODER_LIMITS_HASH = "c6c071df3c80f18702cb0cb41298d56524e2cdba4bbe77d8a9755cfcd95d20c5"
DRAFT_STATUS = "S20_520_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_520_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_520_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_520_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_520_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_520_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (52000, "MERGE_NO_COMMON_ANCESTOR"),
    (52001, "MERGE_ANCESTOR_MISMATCH"),
    (52002, "MERGE_WORKSPACE_MISMATCH"),
    (52003, "MERGE_EPOCH_MISMATCH"),
    (52004, "MERGE_DEPENDENCY_ROOT_CHANGE_UNSUPPORTED"),
    (52005, "MERGE_COMPARE_FAILED"),
    (52006, "MERGE_RESOURCE_LIMIT"),
    (52007, "MERGE_CONFLICT_FORMAT_INVALID"),
    (52008, "MERGE_CONFLICT_DIGEST_MISMATCH"),
    (52009, "MERGE_CONFLICT_CANONICAL_ORDER"),
    (52010, "MERGE_CONFLICT_VERSION_UNSUPPORTED"),
    (52011, "MERGE_PLAN_UNSUPPORTED"),
    (52012, "MERGE_RESULT_MISMATCH"),
    (52013, "MERGE_INTERNAL_INVARIANT"),
)
REASONS = ("AddAdd", "DeleteEdit", "FieldEdit", "KindEdit", "Collateral", "Closure", "RootAnchor", "PolicyRoot", "MetadataEdit")

SPEC_MARKERS = (
    "# Merge v1",
    "Status: S20-520 contract",
    'contract_domain   = "sley2.merge-conflict.v1"',
    "contract_tag      = 520",
    "digest_domain_tag = 21",
    "kind_tag          = 520",
    f"field_schema_hash = {FIELD_SCHEMA_HASH}",
    f"decoder_limits_hash = {DECODER_LIMITS_HASH}",
    "## Merge judgment",
    "| J8 |",
    "## Merged root",
    "## Merge plan",
    "## Conflict object",
    "## Determinism and symmetry",
    "| conflict entries | `131,070` |",
    "`MERGE_RESULT_MISMATCH`; the merge never repairs a mismatch",
)
ADR_MARKERS = (
    "# ADR-0028: Merge composition and conflict boundary",
    "1. **Merge theirs into ours through the frozen commit.**",
    "2. **Judgment over S20-510 deltas only.**",
    "3. **Merged root is re-judged.**",
    "4. **Canonical conflict object.**",
    "5. **Result mismatch fails closed.**",
    "6. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/MERGE_V1.md`", "ADR-0028")
SOURCE_MARKERS = (
    "pub fn find_common_ancestor",
    "pub fn judge_merge",
    "pub fn decode_merge_conflict",
    "const CONTRACT_TAG: u32 = 520;",
    "const DIGEST_DOMAIN_TAG: u32 = 21;",
    "Self::InternalInvariant => 52_013,",
)
PREIMAGE_PATTERN = re.compile(
    r"^(field schema preimage|decoder limits preimage) = (.+)$\n^(field_schema_hash|decoder_limits_hash) = ([0-9a-f]{64})$",
    re.MULTILINE,
)


def recomputed_hashes(spec: str) -> dict[str, tuple[str, str]]:
    found = {label: (preimage, digest) for label, preimage, _, digest in PREIMAGE_PATTERN.findall(spec)}
    if set(found) != {"field schema preimage", "decoder limits preimage"}:
        return {}
    script = "import sys, blake3\nfor line in sys.stdin.read().splitlines():\n    print(blake3.blake3(line.encode('ascii')).hexdigest())\n"
    completed = subprocess.run(
        ["uv", "run", "--project", "oracle/scb1", "--frozen", "python", "-c", script],
        cwd=ROOT,
        input="\n".join(found[label][0] for label in ("field schema preimage", "decoder limits preimage")),
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        return {}
    digests = completed.stdout.split()
    if len(digests) != 2:
        return {}
    return {
        "field schema preimage": (found["field schema preimage"][1], digests[0]),
        "decoder limits preimage": (found["decoder limits preimage"][1], digests[1]),
    }


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES, IDENTIFIERS):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2))
        return 1

    spec = SPEC.read_text(encoding="utf-8")
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in spec:
            problems.append(f"spec-code:{symbol}")
    for index, reason in enumerate(REASONS, start=1):
        if f"| {index} | `{reason}` |" not in spec:
            problems.append(f"spec-reason:{reason}")
    hashes = recomputed_hashes(spec)
    if not hashes:
        problems.append("spec-hash:preimage lines missing or blake3 recomputation unavailable")
    for label, (declared, computed) in hashes.items():
        if declared != computed:
            problems.append(f"spec-hash:{label} declared {declared[:12]} computed {computed[:12]}")
    if hashes.get("field schema preimage", ("", ""))[0] not in ("", FIELD_SCHEMA_HASH):
        problems.append("spec-hash:field schema hash differs from the checker constant")
    if hashes.get("decoder limits preimage", ("", ""))[0] not in ("", DECODER_LIMITS_HASH):
        problems.append("spec-hash:decoder limits hash differs from the checker constant")
    adr = ADR.read_text(encoding="utf-8")
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = WORK_PACKAGES.read_text(encoding="utf-8")
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if "52000 through 52013" not in ERROR_CODES.read_text(encoding="utf-8"):
        problems.append("error-codes:range-sentence")

    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    section = summary.get("merge")
    if not isinstance(section, dict):
        problems.append("machine-summary:merge missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/MERGE_V1.md",
        "adr": "docs/adr/ADR-0028-merge-composition-boundary.md",
        "contract_tag": 520,
        "digest_domain_tag": 21,
        "domain": "sley2.merge-conflict.v1",
        "field_schema_hash": FIELD_SCHEMA_HASH,
        "decoder_limits_hash": DECODER_LIMITS_HASH,
        "judgment_rules": 8,
        "conflict_reasons": len(REASONS),
        "stable_error_codes": len(CODES),
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    present = [str(path.relative_to(ROOT)) for path in IMPLEMENTATION_SURFACES if path.exists()]
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        source = SOURCE.read_text(encoding="utf-8") if SOURCE.exists() else ""
        for marker in SOURCE_MARKERS:
            if marker not in source:
                problems.append(f"source-marker:{marker}")
        for numeric, symbol in CODES:
            if symbol not in source:
                problems.append(f"source-code:{symbol}")
        identifiers = IDENTIFIER_SOURCE.read_text(encoding="utf-8")
        if 'b"sley2.merge-conflict.v1"' not in identifiers or "MergeConflictId" not in identifiers:
            problems.append("identifier-registry:merge-conflict domain missing")
        if "`sley2.merge-conflict.v1`" not in IDENTIFIERS.read_text(encoding="utf-8"):
            problems.append("identifier-spec:merge-conflict row missing")
        if status in (REVIEW_PENDING_STATUS, COMPLETE_STATUS):
            if not (FIXTURE_DIR / "v1/accepted.json").exists():
                problems.append("fixture:missing")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-520-merge-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
        "implementation_present": present,
        "stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

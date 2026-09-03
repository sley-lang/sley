#!/usr/bin/env python3
"""Check the S20-540 repository-exchange contract surface and its stage.

While the contract is a draft (no implementation), the check binds the frozen
identity of the contract text, the ADR, the work-package row, and the
machine-summary section, and it fails closed if implementation surfaces appear
before the summary says the contract is frozen.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/REPOSITORY_EXCHANGE_V1.md"
ADR = ROOT / "docs/adr/ADR-0025-repository-exchange-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
IMPLEMENTATION_SURFACES = (
    ROOT / "crates/sley-repo/src/exchange.rs",
    ROOT / "conformance/repository-exchange",
)

FIELD_SCHEMA_HASH = "a843405be5e34d979bb4889b0e98c152dd01f86d9c614c1afda4cf88dd884e2c"
DECODER_LIMITS_HASH = "570c8c8ab522778ad1bdf00e09845b743de391e42e556221cebfe656a7ab2255"
DRAFT_STATUS = "S20_540_CONTRACT_DRAFT_REVIEW_PENDING"
FROZEN_STATUS = "S20_540_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"

SPEC_MARKERS = (
    'contract_domain   = "sley2.repository-exchange.v1"',
    "contract_tag      = 540",
    "digest_domain_tag = 19",
    "kind_tag          = 540",
    f"field_schema_hash = {FIELD_SCHEMA_HASH}",
    f"decoder_limits_hash = {DECODER_LIMITS_HASH}",
    "EXCHANGE_TARGET_NOT_EMPTY",
    "EXCHANGE_TARGET_INCOMPLETE_MISMATCH",
    "EXCHANGE_ANCESTRY_SURPLUS",
    "`33,554,432`",
    "receipts: `4,096`",
    "digest leaves: `8,194`",
    "initialize_trusted_clone_receipts",
    "initialize_trusted_clone_head",
    "| X-07 |",
    "Numeric codes `54000` through `54019` are exact",
    "`sley-repo -> sley-txn -> sley-store`",
    "an exchange never nests an exchange",
)
ADR_MARKERS = (
    "# ADR-0025: Repository exchange composition and clone trust boundary",
    "`digest_domain_tag` 19",
    "forge-nabu-s20-540-design-20260903T013712-0fb98776",
    "Byte-exact branch install",
    "Rows X-01 through X-07",
)
WORK_PACKAGE_MARKERS = (
    "| S20-540 | 170,500 |",
    "docs/spec/REPOSITORY_EXCHANGE_V1.md",
    "ADR-0025",
    "initialize_trusted_clone",
)


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY):
        if not path.is_file():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    spec = SPEC.read_text(encoding="utf-8") if SPEC.is_file() else ""
    adr = ADR.read_text(encoding="utf-8") if ADR.is_file() else ""
    packages = WORK_PACKAGES.read_text(encoding="utf-8") if WORK_PACKAGES.is_file() else ""
    summary = json.loads(SUMMARY.read_text(encoding="utf-8")) if SUMMARY.is_file() else {}
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")

    section = summary.get("repository_exchange")
    if not isinstance(section, dict):
        problems.append("machine-summary:repository_exchange missing")
        section = {}
    expected = {
        "contract": "docs/spec/REPOSITORY_EXCHANGE_V1.md",
        "adr": "docs/adr/ADR-0025-repository-exchange-boundary.md",
        "contract_tag": 540,
        "digest_domain_tag": 19,
        "kind_tag": 540,
        "field_schema_hash": FIELD_SCHEMA_HASH,
        "decoder_limits_hash": DECODER_LIMITS_HASH,
        "stored_exchange_limit_bytes": 67_108_864,
        "embedded_pack_limit_bytes": 33_554_432,
        "receipt_limit": 4096,
        "branch_limit": 4096,
        "digest_leaf_limit": 8194,
        "failure_range": "54000-54019",
        "interruption_rows": 7,
        "implementation_complete": False,
    }
    for key, value in expected.items():
        actual = section.get(key)
        if type(actual) is not type(value) or actual != value:
            problems.append(f"machine-summary:{key}")
    status = section.get("status")
    if status not in (DRAFT_STATUS, FROZEN_STATUS):
        problems.append("machine-summary:status")
    present = [
        str(path.relative_to(ROOT)) for path in IMPLEMENTATION_SURFACES if path.exists()
    ]
    if present:
        problems.append(f"implementation-before-freeze:{present}")

    print(
        json.dumps(
            {
                "contract": "s20-540-repository-exchange-v1",
                "problems": problems,
                "result": "PASS" if not problems else "FAIL",
                "status": status,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return int(bool(problems))


if __name__ == "__main__":
    raise SystemExit(main())

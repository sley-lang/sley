#!/usr/bin/env python3
"""Check the S20-540 repository-exchange contract surface and its stage.

While the contract is a draft (no implementation), the check binds the frozen
identity of the contract text, the ADR, the work-package row, and the
machine-summary section, and it fails closed if implementation surfaces appear
before the summary says the contract is frozen.
"""

from __future__ import annotations

import json
import re
import subprocess
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
DECODER_LIMITS_HASH = "808eaba936f09b2a938306c538e0dff636a1a6d2617ed0b4298d929891db9d09"
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
    "embedded pack bytes: `16,777,216`",
    "receipts: `4,096`",
    "digest leaves: `8,194`",
    "initialize_trusted_clone_receipts_with_maintenance",
    "initialize_trusted_clone_head_with_maintenance",
    "| X-07 |",
    "Numeric codes `54000` through `54021` are exact",
    "| 54020 | `EXCHANGE_ROOT_CLOSURE` |",
    "| 54021 | `EXCHANGE_WORKSPACE_MISMATCH` |",
    "`maintenance -> refs -> accepted`",
    "orders by `len(branch_name)` and then the raw name bytes",
    "`len(x)` is `uvar(byte_length(x))`",
    "`sley-repo -> sley-txn -> sley-store`",
    "an exchange never nests an exchange",
)
ADR_MARKERS = (
    "# ADR-0025: Repository exchange composition and clone trust boundary",
    "`digest_domain_tag` 19",
    "`16,777,216`",
    "forge-ariadne-s20-540-contract-20260903T014755-7931f8f9",
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


PREIMAGE_PATTERN = re.compile(
    r"^(field schema preimage|decoder limits preimage) = (\S+)$\n^(field_schema_hash|decoder_limits_hash) = ([0-9a-f]{64})$",
    re.MULTILINE,
)


def recomputed_hashes(spec: str) -> dict[str, tuple[str, str]]:
    """Recompute BLAKE3-256 over the spec's preimage texts in the oracle environment."""
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
    recomputed = recomputed_hashes(spec)
    if not recomputed:
        problems.append("spec-hash:preimage lines missing or blake3 recomputation unavailable")
    for label, (declared, computed) in recomputed.items():
        if declared != computed:
            problems.append(f"spec-hash:{label} declared {declared[:12]} computed {computed[:12]}")
    if recomputed.get("field schema preimage", ("", ""))[1] not in ("", FIELD_SCHEMA_HASH):
        problems.append("spec-hash:field schema hash differs from the checker constant")
    if recomputed.get("decoder limits preimage", ("", ""))[1] not in ("", DECODER_LIMITS_HASH):
        problems.append("spec-hash:decoder limits hash differs from the checker constant")
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
        "embedded_pack_limit_bytes": 16_777_216,
        "receipt_limit": 4096,
        "branch_limit": 4096,
        "digest_leaf_limit": 8194,
        "failure_range": "54000-54021",
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

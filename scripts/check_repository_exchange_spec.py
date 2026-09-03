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
IN_PROGRESS_STATUS = "S20_540_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_540_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_540_COMPLETE"
SOURCE = ROOT / "crates/sley-repo/src/exchange.rs"
SOURCE_MARKERS = (
    "pub fn export_repository_exchange",
    "pub fn import_repository_exchange",
    'const LEAF_DOMAIN: &[u8] = b"sley2.repository-exchange-leaf.v1";',
    "const CONTRACT_TAG: u32 = 540;",
    "const DIGEST_DOMAIN_TAG: u32 = 19;",
    "pub const MAX_EMBEDDED_PACK_BYTES: usize = 16_777_216;",
    "pub const MAX_EXCHANGE_RECEIPTS: usize = 4_096;",
    "pub const MAX_EXCHANGE_LEAVES: usize = 8_194;",
    "Self::WorkspaceMismatch => 54_021,",
    "fn install_stage_marker",
    "fn verify_incomplete_clone",
    "acquire_exclusive_repository_maintenance_nonblocking",
)
FIXTURE = ROOT / "conformance/repository-exchange/v1/accepted.json"
FIXTURE_EXPECTED = {
    "repository_exchange_id": "8b3b0e600bca7dde58c26e2eb40b8986c6ff3c152db13772bacfc34d967e41d0",
    "repository_pack_id": "cd6b423ab5bbbc1a8e13326d24dffb8ff537775abb1cb9d4703ebcbbd4a5e3b6",
    "digest_tree_root": "42e27dd7ebf4065f9f4f7d299381e40995e8f181f8c75096022ee8cc9837fd03",
    "stored_bytes": 7754,
    "receipts": 2,
    "branches": 2,
}
TXN_SOURCE = ROOT / "crates/sley-txn/src/repository.rs"
TXN_MARKERS = (
    "pub fn incomplete_clone_marker_present",
    "pub fn initialize_trusted_clone_receipts_with_maintenance",
    "pub fn initialize_trusted_clone_head_with_maintenance",
    "pub fn verify_receipt_against_objects",
)

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
    "length-then-bytes order: one-byte varints (1 through 127) precede",
    "atomically rename it over `exchange/v1/<hex>.stage`",
    "`TXN_INCOMPLETE_CLONE` (`39022`",
    "code table in `ERROR_CODES_V1.md`, whose frozen range then extends to\n`39022`",
    "unless the owned result is an incomplete clone",
    "re-run the complete step-7\n      classification",
    "verifications `2,097,152`",
    "without following symlinks",
    "`<hex>.stage.tmp`",
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
    r"^(field schema preimage|decoder limits preimage) = (.+)$\n^(field_schema_hash|decoder_limits_hash) = ([0-9a-f]{64})$",
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
    if status not in (
        DRAFT_STATUS,
        FROZEN_STATUS,
        IN_PROGRESS_STATUS,
        REVIEW_PENDING_STATUS,
        COMPLETE_STATUS,
    ):
        problems.append("machine-summary:status")
    present = [
        str(path.relative_to(ROOT)) for path in IMPLEMENTATION_SURFACES if path.exists()
    ]
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in (IN_PROGRESS_STATUS, REVIEW_PENDING_STATUS, COMPLETE_STATUS):
        source = SOURCE.read_text(encoding="utf-8") if SOURCE.is_file() else ""
        for marker in SOURCE_MARKERS:
            if marker not in source:
                problems.append(f"source-marker:{marker}")
        txn_source = TXN_SOURCE.read_text(encoding="utf-8") if TXN_SOURCE.is_file() else ""
        for marker in TXN_MARKERS:
            if marker not in txn_source:
                problems.append(f"txn-source-marker:{marker}")
        if "pub(crate) fn list_branches_locked" not in (ROOT / "crates/sley-repo/src/refs.rs").read_text(encoding="utf-8"):
            problems.append("refs-source-marker:list_branches_locked")
        fixture = json.loads(FIXTURE.read_text(encoding="utf-8")) if FIXTURE.is_file() else {}
        vectors = fixture.get("vectors") or [{}]
        for key, value in FIXTURE_EXPECTED.items():
            if vectors[0].get(key) != value:
                problems.append(f"fixture:{key}")
        if fixture.get("contract_tag") != 540 or fixture.get("digest_domain_tag") != 19:
            problems.append("fixture:contract-identity")

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

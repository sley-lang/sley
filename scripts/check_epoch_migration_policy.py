#!/usr/bin/env python3
"""Check the S20-760 epoch migration policy and its stage."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/EPOCH_MIGRATION_POLICY_V1.md"
ADR = ROOT / "docs/adr/ADR-0046-epoch-migration-policy-and-profile-preference.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
EPOCH_CONTRACT = ROOT / "docs/spec/SCHEMA_EPOCH_V1.md"
BOOTSTRAP = ROOT / "conformance/schema-epoch/v1/bootstrap.json"

DRAFT_STATUS = "S20_760_CONTRACT_DRAFT_REVIEW_PENDING"
ACCEPTED_STATUS = "S20_760_POLICY_ACCEPTED"
SUPERSEDED_STATUS = "S20_760_SUPERSEDED"

SPEC_MARKERS = (
    "# Epoch Migration Policy v1",
    "Status: S20-760 contract draft, revision 2",
    "## 1. What forces a new epoch",
    "## 2. What a migration must prove",
    "## 3. Who decides",
    "## 4. Ordering",
    "## 5. What stays true across an epoch",
    "## 6. Epoch 2 candidate agenda, with determinations",
    "| 3a | `contract_assert` (144) execution | **PROFILE** |",
    "| 3b | `test_observe` (145) execution | **EPOCH REQUIRED** |",
    "## 7. Explicit exclusions",
    "## 8. Staging",
    "Profile separation is the preferred alternative to an epoch bump",
)
ADR_MARKERS = (
    "# ADR-0046: profile separation is preferred to an epoch bump, and migrations are additive",
    "1. **Profile separation first.**",
    "2. **Migrations are additive.**",
    "3. **One successor at a time.**",
    "4. **Four approvals.**",
    "5. **An agenda, not a decision.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/EPOCH_MIGRATION_POLICY_V1.md`", "ADR-0046")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def determination_facts() -> list[str]:
    """Verify the tree still supports every section 6 determination.

    A determination that rests on a fact must fail when the fact changes.
    These are the five facts the revision 2 table cites.
    """
    problems: list[str] = []
    ssmc = read(ROOT / "crates/sley-ssmc/src/lib.rs")
    sources = ssmc[ssmc.index("pub enum ContractSource") :]
    sources = sources[: sources.index("}")]
    variants = [
        line.strip().rstrip(",")
        for line in sources.splitlines()
        if line.startswith("    ") and not line.strip().startswith("///") and line.strip()
    ]
    if len(variants) != 4:
        problems.append(f"contract-source-variant-drift:{len(variants)}")
    for tag in ("=> 144,", "=> 145,", "=> 160,", "=> 161,", "=> 162,"):
        if tag not in ssmc:
            problems.append(f"e7-opcode-tag-missing:{tag}")

    schema_hash = "1983bc8d6ad9ac3cb5390853f43959cf2c3dc0ae8e0ca18ca8264ca4960133ae"
    if schema_hash not in read(ROOT / "docs/spec/SSMC1.md"):
        problems.append("ssmc1-descriptor-hash-drift")
    fingerprint = read(ROOT / "crates/sley-ssmc/src/fingerprint.rs")
    segment = fingerprint[
        fingerprint.index("SSMC1_FIELD_SCHEMA_HASH") : fingerprint.index(
            "MAX_FINGERPRINT_PREIMAGE_BYTES"
        )
    ]
    packed = "".join(re.findall(r"0x([0-9a-f]{2})", segment))
    if packed != schema_hash:
        problems.append("field-schema-hash-drift")

    profile = read(ROOT / "docs/spec/CONTRACT_TEST_PROFILE_V1.md")
    if "`contract_assert` is supported with these exact" not in profile:
        problems.append("contract-assert-acceptance-drift")
    if "`test_observe` is rejected in every" not in profile:
        problems.append("test-observe-rejection-drift")

    if "push_u32(&mut preimage, profile.lowering_profile);" not in read(
        ROOT / "crates/sley-vm/src/lib.rs"
    ):
        problems.append("cache-key-profile-binding-drift")
    return problems


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
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, EPOCH_CONTRACT):
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

    summary = json.loads(read(SUMMARY))
    section = summary.get("epoch_migration_policy")
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
    if status not in (DRAFT_STATUS, ACCEPTED_STATUS, SUPERSEDED_STATUS):
        problems.append("machine-summary:status")
    for key, value in (
        ("contract", "docs/spec/EPOCH_MIGRATION_POLICY_V1.md"),
        ("adr", "docs/adr/ADR-0046-epoch-migration-policy-and-profile-preference.md"),
        ("checker", "scripts/check_epoch_migration_policy.py"),
        ("epoch_forcing_facts", 9),
        ("migration_obligations", 7),
        ("profile_separation_preferred", True),
        ("epoch_created", False),
        ("migration_performed", False),
        ("active_epochs", 1),
        ("ga_claimed", False),
    ):
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status == ACCEPTED_STATUS:
        for review in (
            "ariadne_contract_review",
            "nabu_architecture_review",
            "vulcan_surface_review",
        ):
            if section.get(review) != "PASS":
                problems.append(f"machine-summary:{review}")

    # The policy creates no epoch: the tree still carries exactly the epoch-1
    # bootstrap record.
    if BOOTSTRAP.exists():
        bootstrap = json.loads(read(BOOTSTRAP))
        if bootstrap.get("epoch_number") != 1:
            problems.append("conformance:epoch-number")
        if bootstrap.get("predecessor") is not None:
            problems.append("conformance:predecessor")
    else:
        problems.append("missing:conformance/schema-epoch/v1/bootstrap.json")
    if summary.get("schema_epoch", {}).get("status") != "S20_140_COMPLETE":
        problems.append("machine-summary:schema_epoch-status")

    for gate in ("v2", "release-check"):
        if not gate_stays_closed(gate):
            problems.append(f"gate-open:{gate}")

    problems.extend(determination_facts())

    result = {
        "active_epochs": 1,
        "contract": "s20-760-epoch-migration-policy-v1",
        "determinations": {
            "epoch_required": ["contract-kinds", "test-observe", "adapter-replay"],
            "profile": ["fingerprint-requirement", "contract-assert", "effect-and-capability"],
        },
        "epoch_created": False,
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

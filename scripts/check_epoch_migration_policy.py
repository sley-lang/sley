#!/usr/bin/env python3
"""Check the S20-760 epoch migration policy and its stage."""

from __future__ import annotations

import json
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
    "Status: S20-760 contract draft",
    "## 1. What forces a new epoch",
    "## 2. What a migration must prove",
    "## 3. Who decides",
    "## 4. Ordering",
    "## 5. What stays true across an epoch",
    "## 6. Epoch 2 candidate agenda (proposals, not decisions)",
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

    result = {
        "active_epochs": 1,
        "contract": "s20-760-epoch-migration-policy-v1",
        "epoch_created": False,
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

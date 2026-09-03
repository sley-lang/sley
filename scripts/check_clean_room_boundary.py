#!/usr/bin/env python3
"""Check the S20-780 clean-room disposition register and the boundary itself."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGISTER = ROOT / "docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md"
ADR = ROOT / "docs/adr/ADR-0002-clean-room-legacy-boundary.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
LEGACY_ADAPTER = ROOT / "bench/legacy/runner.py"

DRAFT_STATUS = "S20_780_CONTRACT_DRAFT_REVIEW_PENDING"
ACCEPTED_STATUS = "S20_780_REGISTER_ACCEPTED"
CODES = ((78000, "CLEAN_ROOM_BOUNDARY_VIOLATION"), (78001, "DISPOSITION_INCOMPLETE"))
DISPOSITION_FIELDS = (
    "**Purpose**",
    "**Observed evidence**",
    "**Machine-native relevance**",
    "**Security impact**",
    "**New equivalent**",
    "**Decision**",
    "**Acceptance test**",
)
# The frozen predecessor's archive path may appear only in the S20-600 adapter,
# its tests, and the documents that record the freeze.
LEGACY_ARTIFACT = "archive/sley/1.2.0"
ALLOWED_LEGACY_REFERENCES = (
    "bench/legacy/",
    "docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md",
    # This checker names the path in order to search for it.
    "scripts/check_clean_room_boundary.py",
)
SCANNED_TREES = ("crates", "oracle", "bench", "fuzz", "scripts")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (REGISTER, ADR, SUMMARY, WORK_PACKAGES, ERROR_CODES, LEGACY_ADAPTER):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2, sort_keys=True))
        return 1

    register = read(REGISTER)
    entries = re.findall(r"^### 1\.\d+ (.+)$", register, re.M)
    for index, title in enumerate(entries, start=1):
        start = register.index(f"### 1.{index} ")
        end = register.index("### 1.", start + 6) if f"### 1.{index + 1} " in register else register.index("## 2. Mechanical boundary")
        body = register[start:end]
        # The closing entry states that nothing else is reused and carries no
        # per-concept fields.
        if "no reused concept" in title.lower():
            continue
        for field in DISPOSITION_FIELDS:
            if field not in body:
                problems.append(f"disposition-incomplete:1.{index}:{field}")

    # 1. No legacy source or archive reference outside the adapter.
    references = 0
    for tree in SCANNED_TREES:
        for path in sorted((ROOT / tree).rglob("*")):
            if not path.is_file() or path.suffix not in (".rs", ".py", ".toml", ".json"):
                continue
            relative = str(path.relative_to(ROOT))
            if "/target/" in relative or "__pycache__" in relative:
                continue
            if LEGACY_ARTIFACT in read(path):
                references += 1
                if not any(relative.startswith(prefix) for prefix in ALLOWED_LEGACY_REFERENCES):
                    problems.append(f"clean-room-violation:legacy-artifact-reference:{relative}")

    # 2. No crate depends on a legacy package, and every path dependency is a
    #    Sley 2 workspace crate.
    workspace = {path.name for path in (ROOT / "crates").iterdir() if path.is_dir()}
    for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        text = read(manifest)
        for dependency in re.findall(r'^([a-z0-9-]+) = \{ path = "\.\./([a-z0-9-]+)"', text, re.M):
            name, target = dependency
            if target not in workspace:
                problems.append(f"clean-room-violation:path-dependency:{manifest.name}:{target}")
            if name.startswith("sley1") or "legacy" in name:
                problems.append(f"clean-room-violation:legacy-dependency:{manifest.name}:{name}")

    # 3. The one touchpoint runs out of process.
    adapter = read(LEGACY_ADAPTER)
    if "subprocess" not in adapter:
        problems.append("clean-room-violation:adapter-not-out-of-process")

    summary = json.loads(read(SUMMARY))
    section = summary.get("clean_room_disposition_register")
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
    if status not in (DRAFT_STATUS, ACCEPTED_STATUS):
        problems.append("machine-summary:status")
    for key, value in (
        ("contract", "docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md"),
        ("adr", "docs/adr/ADR-0002-clean-room-legacy-boundary.md"),
        ("checker", "scripts/check_clean_room_boundary.py"),
        ("reused_concepts", 2),
        ("legacy_source_in_tree", False),
        ("legacy_dependencies", 0),
        ("similarity_audit_performed", False),
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
    codes = read(ERROR_CODES)
    for number, symbol in CODES:
        if symbol not in codes:
            problems.append(f"error-codes-missing:{number}:{symbol}")
    if "`docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md`" not in read(WORK_PACKAGES):
        problems.append("work-package-missing:register")

    result = {
        "contract": "s20-780-clean-room-disposition-register-v1",
        "legacy_artifact_references": references,
        "problems": problems,
        "register_entries": len(entries),
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

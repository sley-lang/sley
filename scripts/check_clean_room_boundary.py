#!/usr/bin/env python3
"""Check the S20-780 clean-room disposition register and the boundary itself."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGISTER = ROOT / "docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md"
ADR = ROOT / "docs/adr/ADR-0002-clean-room-legacy-boundary.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
LEGACY_ADAPTER = ROOT / "bench/legacy/runner.py"
SECTION2_HEADING = "## 2. Mechanical boundary"

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
LINEAGE_MARKERS = ("reuse", "reimplemented", "not-applicable")
# Every legacy identifier the tree may name: the frozen archive path, both
# artifact filenames, the origin, and both freeze digests.
SENTINELS = (
    "archive/sley/1.2.0",
    "sley-1.2.0-linux-x86_64",
    "sley-1.2.0-source",
    "GreyforgeLabs/sley",
    "b24f19c6a348751c93c9cf63f6f4154f6132796112c26f9d8c0e71324080dbc7",
    "1c866d360305d0b511dc2c33c4907b33544fc73bc6cb6fa4c0e1687df48eb90e",
)
# The exact reference inventory: every tracked file outside the review
# transcripts that may carry a sentinel. Anything else carrying one is a
# violation; an inventory path that is no longer tracked is stale.
SENTINEL_INVENTORY = (
    "bench/legacy/runner.py",
    "bench/legacy/tests/test_runner.py",
    "bench/raw/tests/test_runner.py",
    "bench/benchmark-plan.json",
    "docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md",
    "docs/spec/LEGACY_ARTIFACT_ADAPTER_V1.md",
    "machineresearch/sley-2.0/01-legacy-freeze-and-authority.md",
    "machineresearch/sley-2.0/s20-530-v4-semantic-amendment-design.md",
    "machineresearch/sley-2.0/machine-summary.json",
    "evidence/release/decision-dossier.json",
    "scripts/check_clean_room_boundary.py",
    "scripts/check_legacy_runner.py",
    "scripts/check_s20_530_crash_recovery.py",
    "scripts/verify_s20_530_accepted_state.py",
)
TRANSCRIPT_TREE = "machineresearch/sley-2.0/reviews/"
# Transcripts record reviews and are never imported, but a whole-tree pass
# could hide executable code, so only non-executable transcript suffixes are
# admitted there; anything else under the tree is a violation.
TRANSCRIPT_SUFFIXES = (".log", ".json", ".md")
# The executable files allowed to carry a sentinel: the adapter, its tests,
# and the boundary scripts that name the sentinels to search for them.
EXECUTABLE_INVENTORY = (
    "bench/legacy/runner.py",
    "bench/legacy/tests/test_runner.py",
    "bench/raw/tests/test_runner.py",
    "scripts/check_clean_room_boundary.py",
    "scripts/check_legacy_runner.py",
    "scripts/check_s20_530_crash_recovery.py",
    "scripts/verify_s20_530_accepted_state.py",
)
LEGACY_NAME = re.compile(r"sley1|sley-1|sley_1|legacy")
ENTRY_TITLE = re.compile(r"^### 1\.(\d+) (.+)$", re.M)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def tracked_files() -> list[str]:
    out = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    )
    return [line for line in out.stdout.splitlines() if line]


def field_has_body(body: str, field: str) -> bool:
    """Whether a field label is followed by real content, not just the label."""
    match = re.search(rf"^- {re.escape(field)}:(.*)$", body, re.M)
    if match is None:
        return False
    rest = [match.group(1).strip()]
    tail = body[match.end():]
    for line in tail.splitlines():
        if re.match(r"^- \*\*|\A###? ", line):
            break
        rest.append(line.strip())
    return any(text for text in rest)


def iter_dependencies(manifest: Path) -> list[tuple[str, dict]]:
    try:
        data = tomllib.loads(read(manifest))
    except (OSError, tomllib.TOMLDecodeError):
        return []
    found: list[tuple[str, dict]] = []
    sections = [data.get("dependencies", {}), data.get("dev-dependencies", {}), data.get("build-dependencies", {})]
    for target in data.get("target", {}).values():
        if isinstance(target, dict):
            sections.extend(target.get(key, {}) for key in ("dependencies", "dev-dependencies", "build-dependencies"))
    workspace_deps = data.get("workspace", {}).get("dependencies", {})
    if isinstance(workspace_deps, dict):
        sections.append(workspace_deps)
    for section in sections:
        if not isinstance(section, dict):
            continue
        for name, spec in section.items():
            if isinstance(spec, str):
                found.append((name, {"version": spec}))
            elif isinstance(spec, dict):
                found.append((name, spec))
    return found


def main() -> int:
    problems: list[str] = []
    for path in (REGISTER, ADR, SUMMARY, WORK_PACKAGES, ERROR_CODES, LEGACY_ADAPTER):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2, sort_keys=True))
        return 1

    register = read(REGISTER)
    titles = ENTRY_TITLE.findall(register)
    if not titles:
        problems.append("register:no-section-1-entries")
        entries: list[tuple[str, str, str]] = []
    else:
        matches = list(ENTRY_TITLE.finditer(register))
        section2 = register.find(SECTION2_HEADING)
        if section2 == -1:
            problems.append("register:section-2-heading-missing")
            section2 = len(register)
        entries = []
        for position, match in enumerate(matches):
            start = match.start()
            end = matches[position + 1].start() if position + 1 < len(matches) else section2
            entries.append((match.group(1), match.group(2), register[start:end]))
    reuse_titles: list[str] = []
    reimplemented_titles: list[str] = []
    for index, (number, title, body) in enumerate(entries):
        markers = [marker for marker in LINEAGE_MARKERS if f"**Lineage**: {marker}" in body]
        if len(markers) != 1:
            problems.append(f"disposition-lineage:1.{number}")
            continue
        (marker,) = markers
        if marker == "not-applicable":
            if index != len(entries) - 1:
                problems.append(f"disposition-closure-not-last:1.{number}")
            continue
        for field in DISPOSITION_FIELDS:
            if field not in body:
                problems.append(f"disposition-incomplete:1.{number}:{field}")
            elif not field_has_body(body, field):
                problems.append(f"disposition-empty:1.{number}:{field}")
        if marker == "reuse":
            reuse_titles.append(title)
        else:
            reimplemented_titles.append(title)

    # 1. Sentinel inventory over every tracked file.
    tracked = set(tracked_files())
    for inventory_path in SENTINEL_INVENTORY:
        if inventory_path not in tracked:
            problems.append(f"stale-inventory:{inventory_path}")
    executable_with_sentinel: list[str] = []
    for relative in sorted(tracked):
        if relative.startswith(TRANSCRIPT_TREE):
            if Path(relative).suffix not in TRANSCRIPT_SUFFIXES:
                problems.append(f"clean-room-violation:transcript-executable:{relative}")
            continue
        try:
            text = (ROOT / relative).read_bytes().decode("utf-8", errors="replace")
        except OSError:
            continue
        if not any(sentinel in text for sentinel in SENTINELS):
            continue
        if relative not in SENTINEL_INVENTORY:
            problems.append(f"clean-room-violation:legacy-reference:{relative}")
        if relative.endswith(".py"):
            executable_with_sentinel.append(relative)
    if sorted(executable_with_sentinel) != sorted(EXECUTABLE_INVENTORY):
        problems.append(
            "clean-room-violation:executable-inventory:"
            + ",".join(sorted(set(executable_with_sentinel) ^ set(EXECUTABLE_INVENTORY)))
        )

    # 2. No crate depends on a legacy package, and every path dependency is a
    #    Sley 2 workspace crate.
    workspace = {path.name for path in (ROOT / "crates").iterdir() if path.is_dir()}
    if len(workspace) != 18:
        problems.append(f"clean-room-violation:crate-count:{len(workspace)}")
    manifests = sorted((ROOT / "crates").glob("*/Cargo.toml"))
    root_manifest = ROOT / "Cargo.toml"
    if root_manifest.exists():
        manifests.append(root_manifest)
    fuzz_manifest = ROOT / "fuzz" / "Cargo.toml"
    if fuzz_manifest.exists():
        manifests.append(fuzz_manifest)
    for manifest in manifests:
        relative = manifest.relative_to(ROOT)
        for name, spec in iter_dependencies(manifest):
            target = spec.get("path", "")
            if isinstance(target, str) and target.startswith(".."):
                resolved = (manifest.parent / target).name
                if resolved not in workspace:
                    problems.append(f"clean-room-violation:path-dependency:{relative}:{resolved}")
            if LEGACY_NAME.search(name):
                problems.append(f"clean-room-violation:legacy-dependency:{relative}:{name}")
            version = spec.get("version", "")
            if name == "sley" and isinstance(version, str) and version.startswith("1"):
                problems.append(f"clean-room-violation:legacy-dependency:{relative}:{name}:{version}")
            source = spec.get("git", "")
            if isinstance(source, str) and "GreyforgeLabs/sley" in source:
                problems.append(f"clean-room-violation:legacy-dependency:{relative}:{name}:git")
    lock = ROOT / "Cargo.lock"
    if lock.exists():
        for stanza in read(lock).split("[[package]]")[1:]:
            fields = dict(re.findall(r'^(name|version|source) = "([^"]+)"', stanza, re.M))
            name = fields.get("name", "")
            if LEGACY_NAME.search(name) or (name == "sley" and fields.get("version", "").startswith("1")):
                problems.append(f"clean-room-violation:legacy-locked-dependency:{name}")
            if "GreyforgeLabs/sley" in fields.get("source", ""):
                problems.append(f"clean-room-violation:legacy-locked-dependency:{name}:git")

    # 3. The bounded touchpoint runs out of process.
    adapter = read(LEGACY_ADAPTER)
    if "import subprocess" not in adapter or "subprocess.Popen(" not in adapter:
        problems.append("clean-room-violation:adapter-not-out-of-process")
    if "shell=False" not in adapter:
        problems.append("clean-room-violation:adapter-shell-flag")
    for banned in ("os.system", "ctypes", "dlopen", "os.exec", "eval(", "shell=True"):
        if banned in adapter:
            problems.append(f"clean-room-violation:adapter-in-process:{banned}")

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
        ("reused_concepts", 3),
        ("legacy_source_in_tree", False),
        ("legacy_dependencies", 0),
    ):
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if section.get("reused_concept_entries") != reuse_titles:
        problems.append("machine-summary:reused_concept_entries")
    if section.get("reimplemented_concept_entries") != reimplemented_titles:
        problems.append("machine-summary:reimplemented_concept_entries")
    # The audit fact is recorded, never frozen: requiring False would fail
    # the gate on the day the audit is performed and recorded, mechanically
    # forbidding the control's own remediation.
    if not isinstance(section.get("similarity_audit_performed"), bool):
        problems.append("machine-summary:similarity_audit_performed")
    if status == ACCEPTED_STATUS:
        for review in ("ariadne_review", "nabu_review", "vulcan_review"):
            value = section.get(review)
            if not isinstance(value, str) or not value.startswith("PASS"):
                problems.append(f"machine-summary:{review}")
    codes = read(ERROR_CODES)
    for number, symbol in CODES:
        if symbol not in codes:
            problems.append(f"error-codes-missing:{number}:{symbol}")
    if "`docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md`" not in read(WORK_PACKAGES):
        problems.append("work-package-missing:register")

    result = {
        "contract": "s20-780-clean-room-disposition-register-v1",
        "problems": problems,
        "register_entries": len(entries),
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

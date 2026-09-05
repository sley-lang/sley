#!/usr/bin/env python3
"""Check the S20-260/S20-270 extended opcode profile contract and its slices."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md"
ADR = ROOT / "docs/adr/ADR-0039-vm-extended-opcode-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
VM_LIB = ROOT / "crates/sley-vm/src/lib.rs"
LOWER = ROOT / "crates/sley-vm/src/lower.rs"
EXECUTE = ROOT / "crates/sley-vm/src/execute.rs"
EXTENDED_TESTS = ROOT / "crates/sley-vm/src/extended_tests.rs"

DRAFT_STATUS = "S20_260_270_EXTENDED_CONTRACT_DRAFT_REVIEW_PENDING"
IN_PROGRESS_STATUS = "S20_260_270_EXTENDED_SLICES_IN_PROGRESS"
IMPLEMENTED_STATUS = "S20_260_270_EXTENDED_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_260_270_EXTENDED_COMPLETE"
SLICES = ("E1", "E2", "E3", "E4", "E5", "E6", "E7a")
SLICE_STATUSES = ("PENDING", "IN_PROGRESS", "IMPLEMENTED")
SLICE_OPCODES = {
    "E1": [1, 16, 17, 32, 33, 34, 35, 96, 97, 98, 99, 100, 101, 128, 129, 130, 131],
    "E2": [64, 65, 66, 67, 68, 69, 70, 71],
    "E3": [80, 81, 82, 83, 84, 85],
    "E4": [18, 19, 20, 21, 36, 37, 38, 39, 40],
    "E5": [176, 177, 178, 192, 193, 194],
    "E6": [112],
    "E7a": [144],
}
SPEC_MARKERS = (
    "# VM Extended Opcode Profile v1",
    "Status: S20-260/S20-270 full-profile contract draft",
    "`CacheProfile::EXTENDED_V1` (`lowering_profile = 2`, bytecode `SLEYBC02`)",
    "## 1. Profile and bytecode",
    "## 2. Runtime values",
    "## 3. Signature judgment and semantics by family",
    "### E1 data (1, 16, 17, 32, 33, 34, 35, 96 to 101, 128 to 131)",
    "### E2 checked integers (64 to 71)",
    "### E3 deterministic floats (80 to 85) and float order",
    "### E4 aggregates and maps (18 to 21, 36 to 40)",
    "### E5 cells, hashing, globals, references (176 to 178, 192 to 194)",
    "### E6 direct calls (112)",
    "### E7a contract assertions (144)",
    "### E7 tests, effects, adapters, capabilities (145, 160 to 162)",
    "### 3.1 Judgment without lowering",
    "Normative acceptance invariant: judgment accepts exactly the Functions",
    "The judgment ignores `LoweringInput.state_root`",
    "judgment_acceptance_matches_lowering_acceptance",
    "`Err(BuiltinFailure(ContractViolation, 1))`",
    "stays outside S20-360 phase 7 operation analysis",
    "overflow 1, divide by zero 2, invalid shift 3",
    "`0x7fc00000`, `0x7ff8000000000000`",
    "## 4. Observation and reports",
    "## 6. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0039: the extended opcode profile as a second, explicit VM profile",
    "1. **A second profile, not a mutation.**",
    "2. **The manifest and type checker are the authority.**",
    "3. **Immediates enter the bytecode explicitly**",
    "4. **Family slices.**",
    "5. **Execution-local values never persist.**",
    "6. **E7 waits for its owners, except where an owner already spoke.**",
    "7. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`", "ADR-0039")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, VM_LIB, LOWER, EXECUTE, EXTENDED_TESTS):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2))
        return 1
    spec = read(SPEC)
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("vm_extended_opcode_profile")
    if not isinstance(section, dict):
        problems.append("machine-summary:vm_extended_opcode_profile missing")
        section = {}
    status = section.get("status")
    if status not in (DRAFT_STATUS, IN_PROGRESS_STATUS, IMPLEMENTED_STATUS, COMPLETE_STATUS):
        problems.append("machine-summary:status")
    expected = {
        "contract": "docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md",
        "adr": "docs/adr/ADR-0039-vm-extended-opcode-boundary.md",
        "cache_profile": "EXTENDED_V1",
        "bytecode_magic": "SLEYBC02",
        "restricted_profile_unchanged": True,
        "new_stable_error_codes": 0,
        # Slice E7a landed contract assertions; the rest of E7 stays excluded.
        "e7_excluded": "PARTIAL_E7A_LANDED",
        "e7_opcodes": [145, 160, 161, 162],
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if section.get("e7a_epoch_determination", "").find("no epoch required") < 0:
        problems.append("machine-summary:e7a-determination")
    slices = section.get("slices", {})
    if set(slices) != set(SLICES):
        problems.append("machine-summary:slices")
    for name in SLICES:
        entry = slices.get(name, {})
        if entry.get("status") not in SLICE_STATUSES:
            problems.append(f"machine-summary:slice-status:{name}")
        if entry.get("opcodes") != SLICE_OPCODES[name]:
            problems.append(f"machine-summary:slice-opcodes:{name}")
    landed = [name for name in SLICES if slices.get(name, {}).get("status") == "IMPLEMENTED"]
    started = [name for name in SLICES if slices.get(name, {}).get("status") in ("IN_PROGRESS", "IMPLEMENTED")]
    if status == DRAFT_STATUS and started:
        problems.append("slice-started-under-draft-status")
    if status == IMPLEMENTED_STATUS and landed != list(SLICES):
        problems.append("implemented-status-without-all-slices")

    vm_lib = read(VM_LIB)
    lower = read(LOWER)
    execute = read(EXECUTE)
    profile_present = "EXTENDED_V1" in vm_lib
    if profile_present and not started:
        problems.append("extended-profile-before-slice")
    if started:
        for marker in ("pub const EXTENDED_V1", 'b"SLEYBC02"'):
            if marker not in vm_lib and marker not in lower:
                problems.append(f"crate-marker:{marker}")
        if "lowering_profile: 2" not in vm_lib:
            problems.append("crate-marker:lowering_profile: 2")
        if "fn judge_extended" not in lower:
            problems.append("crate-marker:fn judge_extended")
        if "fn execute_extended" not in execute:
            problems.append("crate-marker:fn execute_extended")
    # The judgment runs the canonical-constant precondition in the same
    # position as lowering (contract section 3.1 invariant): the call must
    # sit inside the judgment entry, after the judgment itself.
    judge_body = lower.split("pub fn judge_function_operations")[1].split("\n}\n")[0]
    if "require_canonical_referenced_constants" not in judge_body:
        problems.append("judgment-missing-canonical-constant-check")
    elif judge_body.index("judge_extended(") > judge_body.index(
        "require_canonical_referenced_constants"
    ):
        problems.append("judgment-canonical-constant-check-misordered")
    extended_tests = read(EXTENDED_TESTS)
    for test in (
        "fn judgment_rejects_non_canonical_referenced_constant",
        "fn judgment_acceptance_matches_lowering_acceptance",
    ):
        if test not in extended_tests:
            problems.append(f"judgment-test-missing:{test}")
    if "supported_opcodes" in section:
        problems.append("machine-summary:restricted-key-misplaced")
    if status == COMPLETE_STATUS:
        for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
            if not str(section.get(key, "")).startswith("PASS"):
                problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-260-270-vm-extended-opcode-profile-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
        "slices_landed": landed,
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

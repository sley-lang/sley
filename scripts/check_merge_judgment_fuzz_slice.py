#!/usr/bin/env python3
"""Drift check for the S20-520 merge-judgment persistent fuzz slice (Section 18.5 surface eleven)."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/merge_judgment.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_merge_judgment_fuzz.py"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_MERGE_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "judge_merge(&ancestor, &ours, &theirs)",
    "a merged judgment must repeat its root",
    "a conflict judgment must repeat its bytes",
    "a judged conflict round-trips",
    "every judgment failure carries a frozen code",
    "MergeErrorCode::ALL.contains(&code)",
    "MAX_OPS: usize = 16",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "merge_judgment"',
    'path = "targets/merge_judgment.rs"',
    'sley-repo = { path = "../crates/sley-repo" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    '"full_s20_700_complete": False',
    '"MERGE_JUDGMENT_OUTCOME_AND_DETERMINISM"',
    "MAX_LEN = 34",
]:
    if marker not in wrapper:
        problems.append(f"wrapper-missing:{marker}")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "merge-persistent-fuzz-smoke:",
    "python3 scripts/check_merge_judgment_fuzz_slice.py",
    "python3 scripts/run_merge_judgment_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_merge_judgment_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("max_script_bytes") != 34:
    problems.append("machine-summary-max-script-bytes-drift")
if slice_status.get("section_18_5_surface") != "merge engine":
    problems.append("machine-summary-surface-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["merge_judgment", "Judgment lane"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-merge-judgment-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "MERGE_JUDGMENT_OUTCOME_AND_DETERMINISM",
            "section_18_5_surface": "merge engine",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

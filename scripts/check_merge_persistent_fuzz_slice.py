#!/usr/bin/env python3
"""Drift check for the S20-520 merge-engine persistent fuzz slice (Section 18.5 surface eleven)."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/merge_conflict_decoder.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_merge_persistent_fuzz.py"
FIXTURE = ROOT / "conformance/merge/v1/accepted.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_MERGE_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "decode_merge_conflict(candidate)",
    "with_rehashed_trailer(payload)",
    "MergeConflictId::derive(&candidate[..preimage_len])",
    "re-encoding a decoded conflict drifted",
    "find_common_ancestor(&ours, &theirs)",
    "ancestor must be the first shared entry of ours",
    "MergeErrorCode::ALL.contains(&code)",
    "SELECTOR_COUNT: u8 = 3",
    "MAX_FUZZ_INPUT_BYTES: usize = 65_536",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "merge_conflict_decoder"',
    'path = "targets/merge_conflict_decoder.rs"',
    'sley-repo = { path = "../crates/sley-repo" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    "conformance/merge/v1/accepted.json",
    '"full_s20_700_complete": False',
    '"MERGE_CONFLICT_DECODER_AND_ANCESTOR_RULE"',
    "MAX_PAYLOAD_LEN = 65_536",
    "SELECTOR_COUNT = 3",
]:
    if marker not in wrapper:
        problems.append(f"wrapper-missing:{marker}")
# Repair round 7 uniform harness markers (REQ-06 wave): locked build,
# host-config owner-lib instrumentation, corpus-coverage gate, executed and
# coverage proof, crash minimization, and persistent corpus discipline.
for marker in [
    "--locked",
    "-Zhost-config",
    "executed_runs",
    "sync_seed_corpus",
    "minimize_crashes",
    "owner_lib_sancov_symbols",
    "corpus_persistent",
    "SLEY_FUZZ_CC",
    "-minimize_crash=1",
    '"source_commit": git_output',
    '"worktree_dirty": bool(git_output',
    "OWNER_RLIB",
    "corpus_file_count",
    "coverage_counters",
    "trace-compares",
    "toolchain_versions",
    "must cover the corpus",
]:
    if marker not in wrapper:
        problems.append(f"wrapper-missing:{marker}")


fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
if fixture.get("contract") != "sley2-merge-v1":
    problems.append("fixture-contract-drift")
if not any(vector.get("outcome") == "conflict" for vector in fixture.get("vectors", [])):
    problems.append("fixture-no-conflict-vector")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "merge-persistent-fuzz-smoke:",
    "python3 scripts/check_merge_persistent_fuzz_slice.py",
    "python3 scripts/run_merge_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_merge_persistent_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("selector_count") != 3:
    problems.append("machine-summary-selector-count-drift")
if slice_status.get("seed_source") != "conformance/merge/v1/accepted.json":
    problems.append("machine-summary-seed-source-drift")
if slice_status.get("section_18_5_surface") != "merge engine":
    problems.append("machine-summary-surface-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["make merge-persistent-fuzz-smoke", "rewrite only the final", "eleventh"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-merge-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "MERGE_CONFLICT_DECODER_AND_ANCESTOR_RULE",
            "section_18_5_surface": "merge engine",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

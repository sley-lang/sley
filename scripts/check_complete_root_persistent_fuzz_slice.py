#!/usr/bin/env python3
"""Drift check for the scoped S20-250 full complete-root judgment persistent fuzz slice."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/complete_root_judgment.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_complete_root_persistent_fuzz.py"
FIXTURE = ROOT / "conformance/complete-entity-impact/v1/accepted.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_COMPLETE_ROOT_JUDGMENT_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "judge_complete_root(&entities, facts)",
    "a passing complete-root judgment must be repeatable",
    "judged index differs from the plain index",
    "an edge escaped the bound inventory",
    "ImpactErrorCode::ALL.contains(&error.code())",
    "MAX_FUZZ_INPUT_BYTES: usize = 4_096",
    "let kind = reader.byte() % 18 + 1;",
    "4 + usize::from(self.byte() % 21)",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "complete_root_judgment"',
    'path = "targets/complete_root_judgment.rs"',
    'sley-query = { path = "../crates/sley-query" }',
    'sley-ssmc = { path = "../crates/sley-ssmc" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    "conformance/complete-entity-impact/v1/accepted.json",
    '"full_s20_700_complete": False',
    '"COMPLETE_ROOT_JUDGMENT_ONLY"',
    "MAX_LEN = 4_096",
    "FLAG_LANES = 4",
    "MAX_SET_MEMBERS = 24",
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
    "must cover the corpus",
]:
    if marker not in wrapper:
        problems.append(f"wrapper-missing:{marker}")


fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
if fixture.get("contract") != "sley2-complete-entity-impact-v1":
    problems.append("fixture-contract-drift")
vectors = fixture.get("vectors") or [{}]
if len(vectors[0].get("request", {}).get("entities", [])) != 19:
    problems.append("fixture-entity-count-drift")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "complete-root-persistent-fuzz-smoke:",
    "python3 scripts/check_complete_root_persistent_fuzz_slice.py",
    "python3 scripts/run_complete_root_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_complete_root_persistent_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("flag_lanes") != 4:
    problems.append("machine-summary-flag-lane-drift")
if slice_status.get("max_set_members") != 24:
    problems.append("machine-summary-set-width-drift")
if slice_status.get("seed_source") != "conformance/complete-entity-impact/v1/accepted.json":
    problems.append("machine-summary-seed-source-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["make complete-root-persistent-fuzz-smoke", "flags byte"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-complete-root-judgment-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "COMPLETE_ROOT_JUDGMENT_ONLY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

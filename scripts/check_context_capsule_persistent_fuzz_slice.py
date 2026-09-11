#!/usr/bin/env python3
"""Drift check for the scoped S20-320 full context capsule builder persistent fuzz slice."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/context_capsule_builder.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_context_capsule_persistent_fuzz.py"
FIXTURE = ROOT / "conformance/context-capsule/v1/accepted.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_CONTEXT_CAPSULE_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "judge_complete_root(&entities, facts)",
    "build_context_capsule(&request, &response)",
    "build_context_capsule_session(&request, &response, session)",
    "ContextCapsuleErrorCode::ALL.contains(&error.code())",
    "capsule drifted between builds",
    "CapsuleCompleteness::Complete",
    "assert_eq!(page.completeness(), CapsuleCompleteness::Page);",
    "MAX_FUZZ_INPUT_BYTES: usize = 4_096",
    "MAX_PAGES: usize = 16",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "context_capsule_builder"',
    'path = "targets/context_capsule_builder.rs"',
    'sley-query = { path = "../crates/sley-query" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    "conformance/context-capsule/v1/accepted.json",
    '"full_s20_700_complete": False',
    '"CONTEXT_CAPSULE_BUILDER_ONLY"',
    "MAX_INPUT_LEN = 4096",
    "QUERY_CLASSES = 19",
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
    "OWNER_RLIB",
    "corpus_file_count",
    "coverage_counters",
    "trace-compares",
    "must cover the corpus",
]:
    if marker not in wrapper:
        problems.append(f"wrapper-missing:{marker}")


fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
if fixture.get("contract") != "sley2-context-capsule-v1":
    problems.append("fixture-contract-drift")
if len(fixture.get("vectors", [])) != 24:
    problems.append("fixture-vector-count-drift")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "context-capsule-persistent-fuzz-smoke:",
    "python3 scripts/check_context_capsule_persistent_fuzz_slice.py",
    "python3 scripts/run_context_capsule_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_context_capsule_persistent_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("query_classes") != 19:
    problems.append("machine-summary-query-class-drift")
if slice_status.get("seed_source") != "conformance/context-capsule/v1/accepted.json":
    problems.append("machine-summary-seed-source-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["make context-capsule-persistent-fuzz-smoke", "never presents a page as complete"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-context-capsule-builder-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "CONTEXT_CAPSULE_BUILDER_ONLY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

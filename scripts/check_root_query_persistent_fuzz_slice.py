#!/usr/bin/env python3
"""Drift check for the scoped S20-310 full root-backed query engine persistent fuzz slice."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/root_query_engine.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_root_query_persistent_fuzz.py"
FIXTURE = ROOT / "conformance/root-backed-query/v1/accepted.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_ROOT_QUERY_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "judge_complete_root(&entities, facts)",
    "build_complete_root_snapshot(epoch, root, &entities, facts)",
    "build_root_query_request(&input, query.clone(), limits, allow, after)",
    "execute_root_query(&input, &request)",
    "RootQueryErrorCode::ALL.contains(&error.code())",
    "root query drifted between runs",
    "truncated without continuation",
    "pages do not union to the total",
    "MAX_FUZZ_INPUT_BYTES: usize = 4_096",
    "MAX_PAGES: usize = 16",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "root_query_engine"',
    'path = "targets/root_query_engine.rs"',
    'sley-query = { path = "../crates/sley-query" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    "conformance/root-backed-query/v1/accepted.json",
    '"full_s20_700_complete": False',
    '"ROOT_QUERY_ENGINE_ONLY"',
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
    '"source_commit": git_output',
    '"worktree_dirty": bool(git_output',
    "OWNER_RLIB",
    "corpus_file_count",
    "coverage_counters",
    "trace-compares",
    "toolchain_versions",
    "worktree_dirty_files",
    "-timeout=30",
    "-rss_limit_mb=2048",
    "must cover the corpus",
]:
    if marker not in wrapper:
        problems.append(f"wrapper-missing:{marker}")


fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
if fixture.get("contract") != "sley2-root-backed-query-v1":
    problems.append("fixture-contract-drift")
if len(fixture.get("vectors", [])) != 23:
    problems.append("fixture-vector-count-drift")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "root-query-persistent-fuzz-smoke:",
    "python3 scripts/check_root_query_persistent_fuzz_slice.py",
    "python3 scripts/run_root_query_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_root_query_persistent_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("query_classes") != 19:
    problems.append("machine-summary-query-class-drift")
if slice_status.get("seed_source") != "conformance/root-backed-query/v1/accepted.json":
    problems.append("machine-summary-seed-source-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["make root-query-persistent-fuzz-smoke", "continuation walk"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-root-query-engine-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "ROOT_QUERY_ENGINE_ONLY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

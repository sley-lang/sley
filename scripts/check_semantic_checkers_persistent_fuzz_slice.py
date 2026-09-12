#!/usr/bin/env python3
"""Drift check for scoped S20-700 semantic-checker persistent fuzz targets."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TYPE_TARGET = ROOT / "fuzz/targets/type_checker.rs"
GRAPH_TARGET = ROOT / "fuzz/targets/ssmc_graph_cfg_checker.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
RUNNER = ROOT / "scripts/run_semantic_checkers_persistent_fuzz.py"
REGRESSION = ROOT / "fuzz/regressions/S20_700_HARNESS_001.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
RESULTS = ROOT / "machineresearch/sley-2.0/14-property-fuzz-and-adversarial-results.md"
GAPS = ROOT / "machineresearch/sley-2.0/25-evidence-gaps.md"
NEGATIVE_RESULTS = ROOT / "machineresearch/sley-2.0/24-negative-results.md"
AUDIT = ROOT / "docs/audits/S20_700_SEMANTIC_CHECKERS_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"
GITIGNORE = ROOT / ".gitignore"

problems: list[str] = []

type_target = TYPE_TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "TypeEnvironment::new(definitions)",
    "environment.check_type(value_type, parameter_count)",
    "environment.traits(value_type)",
    "environment.instantiate_in_scope(",
    "MAX_GENERATED_TYPE_NODES: usize = 512",
    "type-checker judgment was not deterministic",
    "check_consistency(",
    "require_orderable passed while traits failed",
    "a checked closed type failed traits",
    "checked substitution of matching length failed arity",
    "fn is_closed(",
]:
    if marker not in type_target:
        problems.append(f"type-target-missing:{marker}")

graph_target = GRAPH_TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "validate_function_graph(",
    "TEMPLATE_COUNT: u8 = 4",
    "MAX_MUTATIONS: usize = 8",
    "MUTATION_COUNT: u8 = 33",
    "graph/CFG judgment was not deterministic",
    "a graph/CFG base template drifted invalid",
    "failure_class(",
    "expected_for(",
    "CODE_UNIVERSE",
    "escaped with unexpected failure class",
    "GRAPH_DUPLICATE_ENTITY",
    "CFG_ENTRY_INVALID",
    # Repair-round pins: the review-derived failure classes must survive
    # in the target's sets or universe, so a silent revert of the set
    # fixes fails here. Set lines are pinned with their trailing comma
    # (comment text alone does not satisfy the pin).
    '"CFG_RESULT_INDEX",',
    '"CFG_DOMINANCE",',
    '"CFG_UNREACHABLE_VALUE",',
    '"TYPE_PARAMETER_OUT_OF_SCOPE",',
    # Round-4 oracle scope: exact per-arm sets for single mutations,
    # determinism plus code-universe membership for several.
    "CODE_UNIVERSE",
    "applied.len() == 1",
    "escaped with unregistered failure class",
]:
    if marker not in graph_target:
        problems.append(f"graph-target-missing:{marker}")
# The universe must cover every failure string the engine can emit:
# each CfgErrorCode as_str literal has a quoted entry INSIDE the
# CODE_UNIVERSE array body (file-wide presence is insufficient, since
# set lines and comments also carry code strings).
engine_cfg = (ROOT / "crates/sley-check/src/cfg.rs").read_text(encoding="utf-8")
engine_lib = (ROOT / "crates/sley-check/src/lib.rs").read_text(encoding="utf-8")
universe_body = graph_target.split("const CODE_UNIVERSE")[1].split("];")[0]
type_codes_body = graph_target.split("const TYPE_CODES")[1].split("];")[0]
TYPE_CODES_BODY = type_codes_body
import re as _re
for literal in _re.findall(r'Self::\w+ => "([A-Z_0-9]+)"', engine_cfg):
    if f'"{literal}",' not in universe_body:
        problems.append(f"universe-missing:{literal}")
for literal in _re.findall(r'Self::\w+ => "(TYPE_[A-Z_0-9]+)"', engine_lib):
    if f'"{literal}",' not in universe_body and literal not in TYPE_CODES_BODY:
        problems.append(f"universe-missing-type:{literal}")
# Every per-arm set entry must be a universe member (no invented codes).
universe_block = graph_target.split("const CODE_UNIVERSE")[1].split("];")[0]
for literal in _re.findall(r'"([A-Z_0-9]+)"', graph_target.split("fn expected_for")[1].split("fn expected_union")[0] if "fn expected_union" in graph_target else graph_target.split("fn expected_for")[1].split("fn selected_mut")[0]):
    if f'"{literal}"' not in universe_block and literal != "TYPE_CODES":
        problems.append(f"set-not-in-universe:{literal}")
runner_text = RUNNER.read_text(encoding="utf-8")
# Full seed byte lists (prefix-only pins are insufficient: a truncation
# that keeps the prefix would still pass).
for marker in [
    "bytes([0x03, 0x01, 0x1B, 0x00, 0x06])",
    "bytes([0x03, 0x01, 0x13, 0x00, 0x01, 0x00, 0x02, 0x02])",
    "0x01, 0x03, 0x04, 0x00, 0x04, 0x12, 0x00, 0x14, 0x00,",
    "0x00, 0x04, 0x01, 0x00, 0x00, 0x02,",
    "bytes([0x01, 0x02, 0x12, 0x01, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02])",
    "bytes([0x00, 0x02, 0x09, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01])",
    "bytes([0x03, 0x02, 0x17, 0x00, 0x01, 0x00, 0x00, 0x00, 0x11, 0x11, 0x00, 0x00, 0x02])",
    "bytes([0x00, 0x03, 0x1D, 0x00, 0x0E, 0x01, 0x01, 0x00, 0x00, 0x00, 0x07, 0x05, 0x00, 0x03])",
    "bytes([0x01, 0x03, 0x01, 0x00, 0x02, 0x0A, 0x01, 0x00, 0x00, 0x0B, 0x01])",
    "bytes([0x01, 0x02, 0x09, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x10, 0x01, 0x00, 0x02])",
]:
    if marker not in runner_text:
        problems.append(f"runner-seed-missing:{marker}")

for path, body in [(TYPE_TARGET, type_target), (GRAPH_TARGET, graph_target)]:
    for forbidden in ["sley_mutate", "decode_mutation_value", "canonical graph decoder"]:
        if forbidden in body:
            problems.append(f"private-or-parallel-codec:{path.relative_to(ROOT)}:{forbidden}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "type_checker"',
    'path = "targets/type_checker.rs"',
    'name = "ssmc_graph_cfg_checker"',
    'path = "targets/ssmc_graph_cfg_checker.rs"',
    'sley-check = { path = "../crates/sley-check" }',
    'sley-ssmc = { path = "../crates/sley-ssmc" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

runner = RUNNER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    '"canonical_graph_decoder_claimed": False',
    '"private_mutation_codec_used": False',
    '"S20_210_PUBLIC_TYPED_TYPE_CHECKER"',
    '"S20_220_PUBLIC_TYPED_GRAPH_CFG_CHECKER"',
    '"source_commit": git_output(["git", "rev-parse", "HEAD"])',
    '"worktree_dirty": bool(git_output(["git", "status", "--porcelain"]))',
    "S20_700_HARNESS_001.json",
    "range(256)",
    "range(128)",
    "range(33)",
    "range(32)",
    "output_tail(error.stdout)",
]:
    if marker not in runner:
        problems.append(f"runner-missing:{marker}")
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
    "toolchain_versions",
    "worktree_dirty_files",
    "-timeout=30",
    "-rss_limit_mb=2048",
    "corpus_file_counts",
    "runs_floor",
]:
    if marker not in runner:
        problems.append(f"runner-missing:{marker}")


regression = json.loads(REGRESSION.read_text(encoding="utf-8"))
if regression.get("finding_id") != "S20-700-HARNESS-001":
    problems.append("regression-finding-id-drift")
if regression.get("input_hex") != "c2":
    problems.append("regression-input-drift")
if regression.get("classification") != "HARNESS_ONLY_FIXED":
    problems.append("regression-classification-drift")
if regression.get("production_checker_defect") is not False:
    problems.append("regression-production-disposition-drift")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "semantic-checkers-persistent-fuzz-smoke:",
    "python3 scripts/check_semantic_checkers_persistent_fuzz_slice.py",
    "python3 scripts/run_semantic_checkers_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

if "/fuzz/target/" not in GITIGNORE.read_text(encoding="utf-8"):
    problems.append("nested-fuzz-target-not-ignored")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_semantic_checkers_persistent_fuzz_slice", {})
expected = {
    "persistent_fuzz_harness": True,
    "full_s20_700_complete": False,
    "canonical_graph_decoder_claimed": False,
    "private_mutation_codec_used": False,
    "max_input_bytes": 4096,
    "max_generated_type_nodes": 512,
    "type_checker_seed_count": 385,
    "graph_cfg_seed_count": 406,
    "graph_template_count": 4,
    "graph_mutation_class_count": 33,
    "max_graph_mutations_per_input": 8,
    "closed_harness_findings": 1,
}
for key, value in expected.items():
    if slice_status.get(key) != value:
        problems.append(f"machine-summary-drift:{key}")

for path, marker in [
    (RESULTS, "S20-700-HARNESS-001"),
    (RESULTS, "public typed S20-210 type checker"),
    (RESULTS, "public typed S20-220"),
    (RESULTS, "graph/CFG validator runs twice"),
    (GAPS, "typed graph/CFG persistent target"),
    (NEGATIVE_RESULTS, "S20-700-HARNESS-001"),
    (AUDIT, "make semantic-checkers-persistent-fuzz-smoke"),
]:
    if marker not in path.read_text(encoding="utf-8"):
        problems.append(f"doc-missing:{path.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-semantic-checkers-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "PUBLIC_TYPED_S20_210_AND_S20_220_CHECKERS_ONLY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

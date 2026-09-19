#!/usr/bin/env python3
"""Drift check for the scoped S20-510 semantic-delta decoder persistent fuzz slice."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/semantic_delta_decoder.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_semantic_delta_persistent_fuzz.py"
FIXTURE = ROOT / "conformance/semantic-comparison/v1/accepted.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_SEMANTIC_DELTA_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "decode_semantic_delta(candidate)",
    "with_rehashed_trailer(payload)",
    "SemanticDeltaId::derive(&candidate[..preimage_len])",
    "re-encoding a decoded delta drifted",
    "decoder emitted a judgment-only failure code",
    "SELECTOR_COUNT: u8 = 2",
    "MAX_FUZZ_INPUT_BYTES: usize = 65_536",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "semantic_delta_decoder"',
    'path = "targets/semantic_delta_decoder.rs"',
    'sley-repo = { path = "../crates/sley-repo" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    "conformance/semantic-comparison/v1/accepted.json",
    '"full_s20_700_complete": False',
    '"SEMANTIC_DELTA_DECODER_ONLY"',
    "MAX_PAYLOAD_LEN = 65_536",
    "SELECTOR_COUNT = 2",
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
if fixture.get("contract") != "sley2-semantic-comparison-v1":
    problems.append("fixture-contract-drift")
if len(fixture.get("vectors", [])) != 9:
    problems.append("fixture-vector-count-drift")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "semantic-delta-persistent-fuzz-smoke:",
    "python3 scripts/check_semantic_delta_persistent_fuzz_slice.py",
    "python3 scripts/run_semantic_delta_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_semantic_delta_persistent_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("selector_count") != 2:
    problems.append("machine-summary-selector-count-drift")
if slice_status.get("seed_source") != "conformance/semantic-comparison/v1/accepted.json":
    problems.append("machine-summary-seed-source-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["make semantic-delta-persistent-fuzz-smoke", "rewrite only the final"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

# The durable proof record is validated against HEAD (ancestry, lane-input
# freshness over the targets' transitive workspace crates, run floor, crash
# disposition, owner instrumentation): scripts/fuzz_proof_record.py.
import sys as _sys
_sys.path.insert(0, str(ROOT / "scripts"))
from fuzz_proof_record import slice_proof_problems as _slice_proof_problems  # noqa: E402
problems.extend(_slice_proof_problems(ROOT, "s20_700_semantic_delta_persistent_fuzz_slice", "scripts/run_semantic_delta_persistent_fuzz.py", ['semantic_delta_decoder']))

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-semantic-delta-decoder-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "SEMANTIC_DELTA_DECODER_ONLY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

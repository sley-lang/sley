#!/usr/bin/env python3
"""Drift check for the scoped S20-540 repository-exchange persistent fuzz slice."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/repository_exchange_importer.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_exchange_persistent_fuzz.py"
FIXTURE = ROOT / "conformance/repository-exchange/v1/accepted.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_EXCHANGE_IMPORT_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "preflight_repository_exchange(candidate, &verify_entity_object)",
    "import_repository_exchange(&target, candidate, &verify_entity_object)",
    "with_rehashed_exchange_trailer(payload)",
    "RepositoryExchangeId::derive(&candidate[..preimage_len])",
    'assert_eq!(second.code(), "EXCHANGE_TARGET_NOT_EMPTY");',
    "failed repository exchange preflight wrote into the target",
    "SELECTOR_COUNT: u8 = 2",
    "MAX_FUZZ_INPUT_BYTES: usize = 65_536",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "repository_exchange_importer"',
    'path = "targets/repository_exchange_importer.rs"',
    'sley-repo = { path = "../crates/sley-repo" }',
    'sley-state-root = { path = "../crates/sley-state-root" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    "conformance/repository-exchange/v1/accepted.json",
    '"full_s20_700_complete": False',
    '"REPOSITORY_EXCHANGE_IMPORTER_ONLY"',
    "MAX_PAYLOAD_LEN = 65_536",
    "SELECTOR_COUNT = 2",
    'entry = vector["vectors"][0]',
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
if fixture.get("contract") != "sley2-repository-exchange-v1":
    problems.append("fixture-contract-drift")
entry = (fixture.get("vectors") or [{}])[0]
stored = bytes.fromhex(entry.get("exchange_hex", ""))
if len(stored) != entry.get("stored_bytes") or not stored.startswith(b"SLEYSCB1"):
    problems.append("fixture-stored-bytes-drift")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "exchange-persistent-fuzz-smoke:",
    "python3 scripts/check_exchange_persistent_fuzz_slice.py",
    "python3 scripts/run_exchange_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_exchange_persistent_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("selector_count") != 2:
    problems.append("machine-summary-selector-count-drift")
if slice_status.get("seed_source") != "conformance/repository-exchange/v1/accepted.json":
    problems.append("machine-summary-seed-source-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["make exchange-persistent-fuzz-smoke", "rehashed bytes replace only the final"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-exchange-import-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "REPOSITORY_EXCHANGE_IMPORTER_ONLY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

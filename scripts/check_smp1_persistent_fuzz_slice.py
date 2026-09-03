#!/usr/bin/env python3
"""Drift check for the scoped S20-410 SMP1 frame decoder persistent fuzz slice."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/smp1_frame_decoder.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
WRAPPER = ROOT / "scripts/run_smp1_persistent_fuzz.py"
FIXTURE = ROOT / "conformance/smp1/v1/accepted.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_700_SMP1_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "decode_frame(candidate, MAX_FRAME_BYTES)",
    "with_rehashed_trailer(payload)",
    "Hello::decode(candidate)",
    "negotiate(&hello, &server)",
    "re-encoding a decoded frame drifted",
    "handshake identity drifted",
    "ProtocolErrorCode::ALL.contains(&error.code())",
    "SELECTOR_COUNT: u8 = 4",
    "reassemble_stream(&decoded)",
    "streamed body drifted",
    "MAX_FUZZ_INPUT_BYTES: usize = 65_536",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "smp1_frame_decoder"',
    'path = "targets/smp1_frame_decoder.rs"',
    'sley-protocol = { path = "../crates/sley-protocol" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

wrapper = WRAPPER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    "conformance/smp1/v1/accepted.json",
    '"full_s20_700_complete": False',
    '"SMP1_FRAME_HELLO_NEGOTIATION_AND_STREAM_ONLY"',
    "MAX_PAYLOAD_LEN = 65_536",
    "SELECTOR_COUNT = 4",
]:
    if marker not in wrapper:
        problems.append(f"wrapper-missing:{marker}")

fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
if fixture.get("contract") != "sley2-smp1-v1":
    problems.append("fixture-contract-drift")
if len(fixture.get("frames", [])) != 3 or set(fixture.get("hellos", {})) != {"client", "server"}:
    problems.append("fixture-shape-drift")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "smp1-persistent-fuzz-smoke:",
    "python3 scripts/check_smp1_persistent_fuzz_slice.py",
    "python3 scripts/run_smp1_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
slice_status = summary.get("s20_700_smp1_persistent_fuzz_slice", {})
if slice_status.get("persistent_fuzz_harness") is not True:
    problems.append("machine-summary-persistent-harness-not-true")
if slice_status.get("full_s20_700_complete") is not False:
    problems.append("machine-summary-full-s20-700-not-false")
if slice_status.get("selector_count") != 4:
    problems.append("machine-summary-selector-count-drift")
if slice_status.get("seed_source") != "conformance/smp1/v1/accepted.json":
    problems.append("machine-summary-seed-source-drift")

audit = AUDIT.read_text(encoding="utf-8") if AUDIT.is_file() else ""
for marker in ["make smp1-persistent-fuzz-smoke", "rewrite only the final"]:
    if marker not in audit:
        problems.append(f"doc-missing:{AUDIT.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-smp1-frame-decoder-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "SMP1_FRAME_HELLO_NEGOTIATION_AND_STREAM_ONLY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

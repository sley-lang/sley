#!/usr/bin/env python3
"""Drift check for the scoped S20-700 restricted-VM persistent target."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "fuzz/targets/vm_canonical_inputs.rs"
FUZZ_MANIFEST = ROOT / "fuzz/Cargo.toml"
RUNNER = ROOT / "scripts/run_vm_persistent_fuzz.py"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
RESULTS = ROOT / "machineresearch/sley-2.0/14-property-fuzz-and-adversarial-results.md"
GAPS = ROOT / "machineresearch/sley-2.0/25-evidence-gaps.md"
AUDIT = ROOT / "docs/audits/S20_700_VM_INPUT_PERSISTENT_SLICE.md"
MAKEFILE = ROOT / "Makefile"
M1_GATE = ROOT / "scripts/check_m1_gate.py"

problems: list[str] = []

target = TARGET.read_text(encoding="utf-8")
for marker in [
    "LLVMFuzzerTestOneInput",
    "validated_execution_input_hashes(lowering, &request)",
    "execute_function(lowering, request.clone())",
    "derive_observation_id(",
    "VM canonical-input hash judgment was not deterministic",
    "VM execution judgment was not deterministic",
    "a canonical fixture input under normal limits was rejected",
    "a valid fixed VM fixture under normal limits was rejected",
    "cross-profile termination drifted",
    "assert_eq!(hashes.len(), request.inputs.len())",
    "FIXTURE_COUNT: u8 = 9",
    "EXTENDED_FIXTURE_COUNT: u8 = 9",
    "Opcode::ContractAssert",
    "Opcode::MapNew",
    "Opcode::CallDirect",
    "extended-family execution judgment was not deterministic",
    "the restricted profile did not refuse the family program",
    "extended observation identity drifted",
    "Opcode::IntAddChecked",
    "Opcode::FloatAdd",
    "Opcode::CellNew",
    "a family fixture under its own canonical inputs failed to execute",
    "the restricted refusal was not the opcode judgment",
    "code.code(),",
    "LowerErrorCode::OpcodeUnsupported",
    "Fixed-position lane header",
    "extended_family_lane(family_selector, &mut cursor)",
    "canonical_f32_bits",
    "canonical_f64_bits",
    "MAX_FUZZ_INPUT_BYTES: usize = 4096",
    "MAX_RAW_INPUTS: usize = 4",
    "MAX_COLLECTION_ITEMS: usize = 4",
    "MAX_PAYLOAD_BYTES: usize = 32",
    "Opcode::BoolNot",
    "Opcode::BoolAnd",
    "Opcode::BoolOr",
    "Opcode::AdapterInvoke",
    "bridge_sublane(&mut cursor)",
    "frozen_bridge_rows()",
    "bridge lane execution was not deterministic",
    "bridge lane lowering refusal drifted",
    "bridge lane judgment refusal drifted",
    "one fuel short of measured must terminate on fuel",
    "SLY1/BRIDGE/",
]:
    if marker not in target:
        problems.append(f"target-missing:{marker}")
for forbidden in ["decode_bytecode", "execute_bytecode", "RawBytecode"]:
    if forbidden in target:
        problems.append(f"raw-bytecode-surface:{forbidden}")

manifest = FUZZ_MANIFEST.read_text(encoding="utf-8")
for marker in [
    'name = "vm_canonical_inputs"',
    'path = "targets/vm_canonical_inputs.rs"',
    'sley-vm = { path = "../crates/sley-vm" }',
]:
    if marker not in manifest:
        problems.append(f"fuzz-manifest-missing:{marker}")

runner = RUNNER.read_text(encoding="utf-8")
for marker in [
    "libclang_rt.fuzzer-x86_64.a",
    "nightly-2026-02-27",
    '"VM_INPUT_EXTENDED_FAMILY_S20_700_BOUNDARY"',
    "must cover the corpus",
    "Done (\\d+) runs",
    "range(EXTENDED_FIXTURE_COUNT)",
    '"full_s20_270_complete": False',
    '"raw_bytecode_decoder_claimed": False',
    '"raw_bytecode_execution_entrypoint_claimed": False',
    '"source_commit": git_output(["git", "rev-parse", "HEAD"])',
    '"worktree_dirty": bool(git_output(["git", "status", "--porcelain"]))',
    "range(256)",
    "range(FIXTURE_COUNT)",
    "range(6)",
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
    "minimize_crash",
    "OWNER_RLIB",
    "corpus_file_count",
    "coverage_counters",
    "trace-compares",
    "must cover the corpus",
]:
    if marker not in runner:
        problems.append(f"runner-missing:{marker}")


makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "vm-persistent-fuzz-smoke:",
    "python3 scripts/check_vm_persistent_fuzz_slice.py",
    "python3 scripts/run_vm_persistent_fuzz.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

summary_text = MACHINE_SUMMARY.read_text(encoding="utf-8")
summary = json.loads(summary_text)
slice_status = summary.get("s20_700_vm_persistent_fuzz_slice", {})
expected = {
    "persistent_fuzz_harness": True,
    "full_s20_700_complete": False,
    "full_s20_270_complete": False,
    "raw_bytecode_decoder_claimed": False,
    "raw_bytecode_execution_entrypoint_claimed": False,
    "fixture_count": 9,
    "identity_fixture_count": 6,
    "boolean_opcode_fixture_count": 3,
    "extended_family_fixture_count": 9,
    "max_input_bytes": 4096,
    "max_raw_inputs": 4,
    "max_collection_items": 4,
    "max_payload_bytes": 32,
    "generated_seed_count": 788,
}
for key, value in expected.items():
    if slice_status.get(key) != value:
        problems.append(f"machine-summary-drift:{key}")
if slice_status.get("vulcan_review") != "PENDING_S20_700FUZZ_FIX_LANDED_REREVIEW":
    problems.append("machine-summary-vulcan-review-drift")
expected_lanes = [
    "E1 constant reference under the extended profile",
    "E2 checked integer add and divide",
    "E3 deterministic float add",
    "E4 ordered map construction with duplicate-key failure values",
    "E5 per-execution cell write and read",
    "E6 direct call with argument copy and nested callee",
    "E7a contract assertion over a Bool predicate",
    "E8 bridge adapter_invoke over frozen rows with tamper/budget sublane",
]
if slice_status.get("extended_family_lanes") != expected_lanes:
    problems.append("machine-summary-lanes-drift")
if '"VM canonical inputs"' in summary_text:
    problems.append("machine-summary-stale-vm-deferred-surface")

gate = M1_GATE.read_text(encoding="utf-8")
if "future targets for blocked mutation families, merge, and protocol" not in gate:
    problems.append("m1-fuzz-smoke-deferred-surface-drift")

for path, marker in [
    (RESULTS, "VM canonical-input persistent libFuzzer slice"),
    (RESULTS, "do not complete S20-700"),
    (GAPS, "Full-GA S20-240 through S20-270 semantics, adapters, persistent reports, and"),
    (GAPS, "no raw-bytecode decoder"),
    (GAPS, "execution entry"),
    (AUDIT, "make vm-persistent-fuzz-smoke"),
]:
    if marker not in path.read_text(encoding="utf-8"):
        problems.append(f"doc-missing:{path.relative_to(ROOT)}:{marker}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "s20-700-vm-canonical-inputs-persistent-libfuzzer-slice-v1",
            "result": "PASS",
            "scope": "VM_INPUT_EXTENDED_FAMILY_S20_700_BOUNDARY",
            "full_s20_700_complete": False,
        },
        indent=2,
        sort_keys=True,
    )
)

#!/usr/bin/env python3
"""Production-code marker pins for the bootstrap gate and E8 lanes.

Companion to check_bootstrap_profile_1.py (the COVERAGE-mapped,
oracle-independent freeze checker): this script is deliberately NOT
coverage-mapped, so it may read implementation sources the independence
scan forbids to oracles — the same split as
check_vm_extended_opcode_profile.py beside the vm-extended oracle
command.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GATE = ROOT / "crates/sley-vm/src/bootstrap.rs"
CLOSURE = ROOT / "crates/sley-vm/src/bootstrap_closure.rs"
ADVERSARIAL = ROOT / "crates/sley-vm/src/bridge_adversarial.rs"
DECLARATION = ROOT / "crates/sley-check/src/effects.rs"
FUZZ = ROOT / "fuzz/targets/vm_canonical_inputs.rs"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

gate = GATE.read_text(encoding="utf-8")
for marker in [
    "pub fn judge_bootstrap_profile",
    "PERMITTED_BOOTSTRAP_OPCODES",
    "bootstrap_row_ok",
    "admit_bridge_use",
    "referenced",
    "A call-graph cycle: recursion is excluded",
    "single-authority rule",
    "not an execution path",
    "bootstrap_gate_inspects_named_definitions",
    "bootstrap_gate_checks_cell_element_types",
    "bootstrap_gate_checks_bridge_row_and_result_types",
    "bootstrap_gate_checks_constant_value_types",
    "bootstrap_gate_refuses_unreached_inventory",
    "bootstrap_gate_refuses_mutual_recursion",
]:
    if marker not in gate:
        problems.append(f"gate-missing:{marker}")

# Nabu slice-2 C0: bind the frozen record's opcode table to the Rust
# const mechanically. Length/sortedness pins catch gross drift, but a
# same-length sorted substitution in an unpinned slot would pass them;
# only the exact set comparison below closes the freeze-integrity gap.
# (This script is deliberately not coverage-mapped, so reading crates/
# here breaks no oracle-independence rule.)
profile = json.loads((ROOT / "conformance/bootstrap-profile/v1/profile.json").read_text(encoding="utf-8"))
match = re.search(r"PERMITTED_BOOTSTRAP_OPCODES: &\[u32\] = &\[(.*?)\];", gate, re.DOTALL)
if match is None:
    problems.append("gate-table-unparseable")
else:
    rust_tags = sorted(int(tag) for tag in re.findall(r"\d+", match.group(1)))
    if rust_tags != sorted(profile["permitted_opcodes"]):
        problems.append("gate-table-record-mismatch")
    if rust_tags != sorted(set(rust_tags)) or 161 in rust_tags:
        problems.append("gate-table-malformed")

closure = CLOSURE.read_text(encoding="utf-8")
for marker in [
    "emit_bootstrap_profile_vectors_for_freeze",
    # Versioned gate inputs (AR-08): frozen v1 workloads judge under V1,
    # the successor replay judges under V2 — the admission-before-evidence
    # property holds per version, never unversioned.
    "judge_bootstrap_profile(&workload.program.gate_input(BootstrapProfileVersion::V1))",
    "profile_version: BootstrapProfileVersion::V2,",
    "closure_workloads_replay_through_v2_with_attribution",
    "Gate admission precedes emission",
    "BOOTSTRAP_PROFILE_VECTOR|",
    "graph_worklist_dfs",
    "image_assemble_emit",
    "variant-switch-exhaustive",
    "trap_on_violation_traps_exactly",
    "closure_workloads_are_gate_admitted",
    "closure_workloads_execute_to_exact_terminations",
]:
    if marker not in closure:
        problems.append(f"closure-missing:{marker}")

adversarial = ADVERSARIAL.read_text(encoding="utf-8")
for marker in [
    "bridge_adversarial_lane_is_deterministic_and_fail_closed",
    "bridge_empty_inputs_cross_as_empty",
    "bridge_capacity_boundary_is_exact_at_two_to_twenty",
    "judge_function_operations",
]:
    if marker not in adversarial:
        problems.append(f"execution-lane-missing:{marker}")

declaration = DECLARATION.read_text(encoding="utf-8")
for marker in [
    "pure_declaration_adversarial",
    "declaration_mutants_fail_identically_used_and_unused",
    "effect_carrying_row_never_serves_pure_invocation",
    "duplicate_adapter_identities_fail_at_index_build",
]:
    if marker not in declaration:
        problems.append(f"declaration-lane-missing:{marker}")

fuzz_target = FUZZ.read_text(encoding="utf-8")
for marker in [
    "bridge_sublane(&mut cursor)",
    "frozen_bridge_rows()",
    "Opcode::AdapterInvoke",
    "EXTENDED_FIXTURE_COUNT: u8 = 9",
]:
    if marker not in fuzz_target:
        problems.append(f"fuzz-lane-missing:{marker}")

makefile = MAKEFILE.read_text(encoding="utf-8")
if "python3 scripts/check_bootstrap_gate_markers.py" not in makefile:
    problems.append("makefile-missing:gate-markers")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "sley2-bootstrap-profile-1",
            "result": "PASS",
            "scope": "BOOTSTRAP_GATE_AND_LANE_CODE_MARKERS",
        },
        indent=2,
        sort_keys=True,
    )
)

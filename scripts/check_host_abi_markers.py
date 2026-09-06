#!/usr/bin/env python3
"""Production-code marker pins for the host ABI freeze.

Companion to check_host_abi_v1.py (the COVERAGE-mapped,
oracle-independent freeze checker): this script is deliberately NOT
coverage-mapped, so it may read implementation sources the independence
scan forbids to oracles — the same split as
check_bootstrap_gate_markers.py beside its oracle command.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE = ROOT / "crates/sley-vm/src/host_abi.rs"
LIB = ROOT / "crates/sley-vm/src/lib.rs"
EXECUTE = ROOT / "crates/sley-vm/src/execute.rs"
EXTENDED = ROOT / "crates/sley-vm/src/extended.rs"
GATE = ROOT / "crates/sley-vm/src/bootstrap.rs"
FIXTURES = ROOT / "crates/sley-vm/tests/rw070_host_abi_freeze.rs"
RECORD = ROOT / "conformance/host-abi/v1/host-abi.json"
MAKEFILE = ROOT / "Makefile"

problems: list[str] = []

module = MODULE.read_text(encoding="utf-8")
for marker in [
    "pub const HOST_ABI_IDENTITY",
    '"HOST_ABI_V1"',
    "pub const HOST_ABI_CONTRACT",
    '"sley2-host-abi-1"',
    "pub const BRIDGE_CODE_B2V1",
    "pub const BRIDGE_CODE_V2B1",
    "pub const BRIDGE_CODE_PSH1",
    "pub const HOST_ABI_BRIDGE_MAX_ITEMS",
    "pub const HOST_ABI_BRIDGE_ELEMENT_FUEL",
    "pub const HOST_ABI_BRIDGE_CAPACITY_CODE",
    "pub const IMAGE_MAGIC_SLEYBC02",
    "pub const IMAGE_VERSION",
    "pub const IMAGE_MAX_BYTES",
    "pub fn check_image_prefix",
    "pub fn load_image",
    "pub fn image_digest",
    "pub struct LoadedImage",
    "pub enum ImageError",
    "IMAGE_UNKNOWN_MAGIC",
    "IMAGE_UNSUPPORTED_VERSION",
    "IMAGE_TRUNCATED",
    "IMAGE_OVERSIZED",
    "IMAGE_TRAILING_DATA",
    "IMAGE_MALFORMED",
    "IMAGE_DIGEST_MISMATCH",
    "IMAGE_BINDING_MISMATCH",
    "pub const fn bridge_identity",
    "host_abi_constants_bind_the_landed_bridge_values",
]:
    if marker not in module:
        problems.append(f"module-missing:{marker}")

# The manifest constants bind the landed values mechanically: a quiet
# same-shape substitution in the module would pass the marker pins above,
# only the exact value comparison below closes the freeze-integrity gap.
record = json.loads(RECORD.read_text(encoding="utf-8"))
value_pins = [
    ("HOST_ABI_BRIDGE_MAX_ITEMS", 1_048_576),
    ("HOST_ABI_BRIDGE_ELEMENT_FUEL", 1),
    ("HOST_ABI_BRIDGE_CAPACITY_CODE", 2),
    ("IMAGE_VERSION", 1),
    ("IMAGE_MAX_BYTES", 67_108_864),
    ("IMAGE_MIN_BYTES", 12),
]
for name, value in value_pins:
    match = re.search(rf"pub const {name}[^=]*=\s*([^;]+);", module)
    if match is None:
        problems.append(f"module-value-unparseable:{name}")
        continue
    digits = re.sub(r"[^0-9]", "", match.group(1))
    if digits != str(value):
        problems.append(f"module-value-mismatch:{name}")
if 'pub const IMAGE_MAGIC_SLEYBC02' not in module or 'b"SLEYBC02"' not in module:
    problems.append("module-magic-mismatch")
for code in ("B2V1", "V2B1", "PSH1"):
    if f'*b"{code}"' not in module:
        problems.append(f"module-code-missing:{code}")

if "pub mod host_abi;" not in LIB.read_text(encoding="utf-8"):
    problems.append("lib-module-missing")

execute = EXECUTE.read_text(encoding="utf-8")
for marker in [
    "pub fn execute_loaded_image",
    "pub struct ApprovedImage",
    "pub struct LoadedExecutionInput",
    "pub enum LoadedExecutionError",
    "fn execute_core",
    "fn validate_loaded_inputs",
    "struct ExecutionSource",
]:
    if marker not in execute:
        problems.append(f"execute-missing:{marker}")

extended = EXTENDED.read_text(encoding="utf-8")
for marker in [
    "pub(crate) fn resolve_push_row",
    "pub(crate) fn has_push_row",
    "fn is_push_row",
]:
    if marker not in extended:
        problems.append(f"extended-missing:{marker}")

gate = GATE.read_text(encoding="utf-8")
for marker in [
    "fn bootstrap_row_ok",
    "pub fn imports(",
]:
    if marker not in gate:
        problems.append(f"gate-missing:{marker}")

fixtures = FIXTURES.read_text(encoding="utf-8")
for test in [
    "rw070_b2v1_exact_invocation_succeeds",
    "rw070_v2b1_exact_invocation_succeeds",
    "rw070_push_u8_exact_invocation_succeeds",
    "rw070_composed_bridge_round_trip_with_growth",
    "rw070_bridge_capacity_refusal_is_typed_index_code_2",
    "rw070_valid_image_loads_and_executes_deterministically",
    "rw070_cache_identity_is_stable_and_bound",
    "rw070_unknown_primitive_identity_refuses",
    "rw070_wrong_abi_version_refuses",
    "rw070_foreign_adapter_identity_refuses",
    "rw070_effectful_bridge_row_refuses",
    "rw070_wrong_request_schema_refuses",
    "rw070_wrong_result_schema_refuses",
    "rw070_v2b1_rejects_non_u8_width",
    "rw070_bridge_under_restricted_profile_refuses",
    "rw070_wrong_vm_version_refuses_cache_key",
    "rw070_nonzero_abi_flags_refuse_cache_key",
    "rw070_image_corruption_is_detected_by_identity_mismatch",
    "rw070_image_truncation_refuses",
    "rw070_image_trailing_data_changes_identity",
    "rw070_image_wrong_magic_and_version_refuse",
    "rw070_oversized_image_refuses",
    "rw070_native_compiler_services_are_not_admitted",
    "rw070_direct_helper_injection_refuses",
    "rw070_development_abi_version_zero_refuses_as_production",
    "rw070_gate_refuses_unreferenced_import_rows",
    "rw070_gate_denies_unknown_imports_like_lowering",
    "rw070_loaded_image_round_trips_lowered_model",
    "rw070_loader_rejects_tamper_through_itself",
    "rw070_loader_rejects_malformed_tags",
    "rw070_push_rows_select_per_use_at_lowering",
    "rw070_gate_refuses_second_push_row",
    "rw070_combined_push_types_share_one_closure",
    "rw070_loaded_execution_matches_lowering_path",
    "rw070_loaded_execution_verifies_manifest_identity",
    "rw070_loaded_execution_binds_epoch_and_inputs",
    "rw070_cancellation_is_deterministic_not_silent",
]:
    if test not in fixtures:
        problems.append(f"fixture-missing:{test}")
if record.get("fixture_index", {}).get("conformance_tests") != 37:
    problems.append("record-fixture-count-mismatch")

# Source hygiene: the bootstrap closure crates carry no host-escape surface
# outside tests. The `#![forbid(unsafe_code)]` line is the allowlisted
# occurrence of the token `unsafe`.
CLOSURE = [
    ROOT / "crates/sley-vm/src",
    ROOT / "crates/sley-check/src",
    ROOT / "crates/sley-ssmc/src",
    ROOT / "crates/sley-id/src",
    ROOT / "crates/sley-mutate/src",
]
FORBIDDEN = [
    'extern "C"',
    "dlopen",
    "libloading",
    "std::fs",
    "std::env",
    "std::process",
    "std::net",
    "Command::new",
]
for crate_dir in CLOSURE:
    for path in sorted(crate_dir.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        production = text.split("#[cfg(test)]")[0]
        for token in FORBIDDEN:
            if token in production:
                problems.append(f"hygiene:{path.relative_to(ROOT)}:{token}")
        for line in production.splitlines():
            stripped = line.strip()
            if "unsafe" in stripped and stripped != "#![forbid(unsafe_code)]":
                problems.append(f"hygiene:{path.relative_to(ROOT)}:unsafe")
                break
        lowered = production.lower()
        if "cargo" in lowered or "rustc" in lowered:
            problems.append(f"hygiene:{path.relative_to(ROOT)}:toolchain-ref")

makefile = MAKEFILE.read_text(encoding="utf-8")
if "python3 scripts/check_host_abi_markers.py" not in makefile:
    problems.append("makefile-missing:check_host_abi_markers")

if problems:
    raise SystemExit("\n".join(problems))

print(json.dumps({"contract": "sley2-host-abi-1", "markers": "PINNED", "result": "PASS"}, indent=2, sort_keys=True))

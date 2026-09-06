#!/usr/bin/env python3
"""Production-code marker pins for EXEC_PACKAGE_V1 / RAW_HASH_V1 (RW-075).

Not coverage-mapped (it reads implementation sources by design): the same
split as `scripts/check_host_abi_markers.py` beside `check_host_abi_v1.py`.
Pins the Rust constants to the frozen records and enforces the AR-01/AR-02
anti-shortcut absence (no semantic service calls on the package/raw-hash
path). Runs in `make quick` directly after `check_exec_package_v1.py`.
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXEC_RS = ROOT / "crates/sley-vm/src/exec_package.rs"
HASH_RS = ROOT / "crates/sley-vm/src/raw_hash.rs"
EXECUTE_RS = ROOT / "crates/sley-vm/src/execute.rs"
CHECK_RS = ROOT / "crates/sley-check/src/lib.rs"
BOOTSTRAP_RS = ROOT / "crates/sley-vm/src/bootstrap.rs"
AUTHORITY_RS = ROOT / "crates/sley-vm/src/admission_authority.rs"
LIB_RS = ROOT / "crates/sley-vm/src/lib.rs"

problems: list[str] = []

exec_rs = EXEC_RS.read_text(encoding="utf-8")
for pin, value in [
    ("EXEC_PACKAGE_IDENTITY", '"EXEC_PACKAGE_V1"'),
    ("EXEC_PACKAGE_CONTRACT", '"sley2-exec-package-1"'),
    ("EXEC_PACKAGE_VERSION", ": u32 = 1"),
    ("EXEC_PACKAGE_MAGIC", 'b"SLEYPKG1"'),
    ("BOOTSTRAP_PROFILE_1_DIGEST", "0x4f, 0x26, 0x91, 0x50"),
    ("BOOTSTRAP_PROFILE_2_DIGEST", "0xfb, 0x2d, 0x8c, 0xc8"),
    ("EXEC_PACKAGE_V2_IDENTITY", '"EXEC_PACKAGE_V2"'),
    ("EXEC_PACKAGE_V2_CONTRACT", '"sley2-exec-package-2"'),
    ("EXEC_PACKAGE_V2_VERSION", ": u32 = 2"),
    ("EXEC_PACKAGE_MAX_BYTES", "67_108_864"),
    ("EXEC_PACKAGE_MAX_DEPENDENCY_BYTES", "8_388_608"),
    ("&BootstrapProfileReport", "&BootstrapProfileReport"),
    ("pub fn package_digests_v2(", "BOOTSTRAP_PROFILE_2_DIGEST"),
    ("pub(crate) fn admit_package_v2(", "HOST_ABI_V2_VERSION"),
    ("pub fn approve_package_v2(", "BOOTSTRAP_PROFILE_2_DIGEST"),
    ("pub fn verify_package_binding_v2(", "package_digests_v2"),
]:
    if pin not in exec_rs or value not in exec_rs:
        problems.append(f"exec-rs-pin-missing:{pin}")

hash_rs = HASH_RS.read_text(encoding="utf-8")
for pin, value in [
    ("RAW_HASH_IDENTITY", '"RAW_BLAKE3_V1"'),
    ("RAW_HASH_CONTRACT", '"sley2-raw-hash-1"'),
    ("RAW_HASH_VERSION", ": u32 = 1"),
    ("RAW_HASH_MAX_BYTES", "1_048_576"),
]:
    if pin not in hash_rs or value not in hash_rs:
        problems.append(f"hash-rs-pin-missing:{pin}")

execute_rs = EXECUTE_RS.read_text(encoding="utf-8")
if "pub fn execute_approved_package(" not in execute_rs:
    problems.append("execute-rs-missing:execute_approved_package")
if "SLEYPOBS1" not in execute_rs:
    problems.append("execute-rs-missing:SLEYPOBS1")
if "fn validate_package_inputs_structural(" not in execute_rs:
    problems.append("execute-rs-missing:structural-validation")

check_rs = CHECK_RS.read_text(encoding="utf-8")
if "pub fn hydrate_verified_definitions(" not in check_rs:
    problems.append("check-rs-missing:hydrate_verified_definitions")

bootstrap_rs = BOOTSTRAP_RS.read_text(encoding="utf-8")
if "#[non_exhaustive]" not in bootstrap_rs or "pub struct BootstrapProfileReport" not in bootstrap_rs:
    problems.append("bootstrap-rs-missing:sealed-report")
if "pub fn closure_fingerprints(" not in bootstrap_rs:
    problems.append("bootstrap-rs-missing:fingerprint-accessor")
if "pub fn admitted_image_digest(" not in bootstrap_rs:
    problems.append("bootstrap-rs-missing:image-digest-accessor")
if "presented_image_bytes" not in bootstrap_rs:
    problems.append("bootstrap-rs-missing:presented-image")
if "fingerprint_function(" not in bootstrap_rs:
    problems.append("bootstrap-rs-missing:gate-fingerprint")
if "fn judged_closure_fingerprints(" not in bootstrap_rs:
    problems.append("bootstrap-rs-missing:narrowing-rule")
if "gate.operation_count()" not in exec_rs or "gate.bridge_uses()" not in exec_rs:
    problems.append("exec-rs-missing:gate-count-binding")
if exec_rs.count("if output.len() > EXEC_PACKAGE_MAX_DEPENDENCY_BYTES") < 4:
    problems.append("exec-rs-missing:incremental-dependency-ceiling")
if "MAX_TYPE_DEPTH" not in exec_rs:
    problems.append("exec-rs-missing:type-nesting-bound")

# Staged v2 admission authority: the one production location permitted to
# call the gate plus the reference lowerer on the admission path (exact
# Sley-driver model; the package/execution/host-ABI/bridge modules stay
# free of those calls per the anti-shortcut pins below). The raw v2
# constructor is crate-private with no public re-export, so reviewed
# integration paths mint exclusively through the authority.
authority_rs = AUTHORITY_RS.read_text(encoding="utf-8")
for marker in [
    "pub fn admit_v2_package(",
    "pub struct V2Closure",
    "judge_bootstrap_profile(",
    "lower_function(",
    "admit_package_v2(",
    "approve_package_v2(",
    "AUTHORITY_REFERENCE_MISMATCH",
    "AUTHORITY_GATE_REFUSED",
    "pub enum AuthorityError",
]:
    if marker not in authority_rs:
        problems.append(f"authority-rs-missing:{marker}")
lib_rs = LIB_RS.read_text(encoding="utf-8")
if "pub mod admission_authority;" not in lib_rs:
    problems.append("lib-module-missing:admission_authority")
if "admit_package_v2" in lib_rs:
    problems.append("lib-reexport-forbidden:admit_package_v2")

# Receipt forgery closure: `AdmissionReceipt` is sealed (`non_exhaustive`
# plus private fields) with controlled accessors, so no downstream struct
# literal or mutation can forge one. The raw v2 constructor is
# crate-private; reviewed production paths mint exclusively through the
# staged authority (textual exclusivity pin below).
if "#[non_exhaustive]" not in exec_rs or "pub struct AdmissionReceipt" not in exec_rs:
    problems.append("exec-rs-missing:sealed-receipt")
for marker in [
    "pub const fn package_digest(",
    "pub const fn profile_digest(",
    "pub const fn host_abi_version(",
    "pub(crate) fn admit_package_v2(",
]:
    if marker not in exec_rs:
        problems.append(f"exec-rs-missing:{marker}")
SRC = ROOT / "crates/sley-vm/src"
for path in sorted(SRC.glob("*.rs")):
    if path.name in ("exec_package.rs", "admission_authority.rs"):
        continue
    production = path.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    if "admit_package_v2(" in production:
        problems.append(f"minter-exclusivity:{path.name}")
    if "AdmissionReceipt {" in production:
        problems.append(f"receipt-literal:{path.name}")

# Anti-shortcut absence: the package/raw-hash sources must not call native
# semantic digest or compiler services (behavioral probes pin this in Rust
# too; this marker makes it a `make quick` gate).
for forbidden in [
    "TypeEnvironment::new",
    "check_constant(",
    "require_hashable(",
    "require_orderable(",
    "fingerprint_function(",
    "fingerprint_type_definition(",
    "verify_fingerprint_claim(",
    "judge_bootstrap_profile(",
    "judge_function_operations(",
    "judge_extended_operation(",
    "lower_function(",
    "fn candidate_digest",
    "fn object_id",
    "fn validate_and_hash_object",
    "resolve_dependency(",
    "build_closure(",
]:
    if forbidden in exec_rs:
        problems.append(f"exec-rs-forbidden:{forbidden}")
for forbidden in [
    "use sley_check",
    "use sley_ssmc",
    "use sley_mutate",
    "TypeEnvironment",
    "FunctionGraph",
    "fingerprint::",
    "Fingerprint",
    "fn candidate_digest",
    "fn object_id",
    "fn validate_and_hash",
]:
    if forbidden in hash_rs:
        problems.append(f"hash-rs-forbidden:{forbidden}")

if problems:
    print("EXEC_PACKAGE_MARKERS: FAIL")
    for problem in problems:
        print(f"  - {problem}")
    raise SystemExit(1)
print("EXEC_PACKAGE_MARKERS: PASS")

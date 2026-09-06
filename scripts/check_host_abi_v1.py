#!/usr/bin/env python3
"""Freeze check for HOST_ABI_V1 (RW-070).

Coverage-mapped and therefore oracle-independent: this script never reads
implementation sources. Production-code marker pins live in
`scripts/check_host_abi_markers.py` (not coverage-mapped), the same split
as the bootstrap-profile checker beside its gate-marker script.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/spec/HOST_ABI_V1.md"
RECORD = ROOT / "conformance/host-abi/v1/host-abi.json"
SUMS = ROOT / "conformance/host-abi/v1/SHA256SUMS"
PROFILE = ROOT / "conformance/bootstrap-profile/v1/profile.json"
BOUNDARY = ROOT / "host-boundary.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
MAKEFILE = ROOT / "Makefile"
REPORT_BUILDER = ROOT / "scripts/build_independent_conformance_report.py"
REVIEWS = ROOT / "machineresearch/sley-2.0/reviews"

DIGEST = "e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2"

problems: list[str] = []

doc = DOC.read_text(encoding="utf-8")
for marker in [
    "HOST_ABI_V1",
    DIGEST,
    "sley2-host-abi-1",
    "default-deny",
    "execute_loaded_image",
    "IMAGE_DIGEST_MISMATCH",
    "SLEYBC02",
    "IMAGE_UNKNOWN_MAGIC",
    "BOOTSTRAP_PROFILE_1",
    "4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630",
    "d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a",
    "Image loading is not compilation",
    "seed absence",
]:
    if marker not in doc:
        problems.append(f"doc-missing:{marker}")

record_bytes = RECORD.read_bytes()
record = json.loads(record_bytes.decode("utf-8"))
digest = hashlib.sha256(record_bytes).hexdigest()
if digest != DIGEST:
    problems.append("record-digest-mismatch")
if digest not in doc:
    problems.append("doc-digest-mismatch")
if record.get("identity") != "HOST_ABI_V1" or record.get("version") != 1:
    problems.append("record-identity-drift")
if record.get("contract") != "sley2-host-abi-1":
    problems.append("record-contract-drift")

rows = record.get("imports", {}).get("rows", [])
if len(rows) != 3:
    problems.append("imports-row-count-drift")
expected_identities = {
    "B2V1": "534c59312f4252494447452f4232563100000000000000000000000000000000",
    "V2B1": "534c59312f4252494447452f5632423100000000000000000000000000000000",
    "PSH1": "534c59312f4252494447452f5053483100000000000000000000000000000000",
}
for row in rows:
    code = row.get("code")
    if row.get("identity_hex") != expected_identities.get(code):
        problems.append(f"imports-identity-drift:{code}")
    if row.get("side_effects") != "none":
        problems.append(f"imports-side-effects-drift:{code}")
if {row.get("code") for row in rows} != {"B2V1", "V2B1", "PSH1"}:
    problems.append("imports-codes-drift")

bindings = record.get("bindings", {})
profile_bytes = PROFILE.read_bytes()
if bindings.get("bootstrap_profile_digest") != hashlib.sha256(profile_bytes).hexdigest():
    problems.append("bindings-profile-digest-drift")
if bindings.get("host_boundary_digest") != hashlib.sha256(BOUNDARY.read_bytes()).hexdigest():
    problems.append("bindings-boundary-digest-drift")
if bindings.get("vm_version") != [1, 0, 0] or bindings.get("lowering_profile") != 2:
    problems.append("bindings-vm-drift")
if bindings.get("lowerer_version") != [2, 0, 0]:
    problems.append("bindings-lowerer-drift")

image = record.get("image", {})
if image.get("magic") != "SLEYBC02" or image.get("format_version") != 1:
    problems.append("image-magic-version-drift")
if image.get("max_bytes") != 67_108_864 or image.get("min_bytes") != 12:
    problems.append("image-bounds-drift")

fixtures = record.get("fixture_index", {})
if fixtures.get("conformance_tests") != 37:
    problems.append("fixture-count-drift")
for key in ("gate_tests", "lane_tests", "real_world_fixture"):
    if not fixtures.get(key):
        problems.append(f"fixture-index-drift:{key}")

sums = SUMS.read_text(encoding="utf-8")
if f"{digest}  host-abi.json\n" not in sums:
    problems.append("sums-drift:host-abi.json")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
frozen = summary.get("host_abi_1", {})
expected = {
    "status": "HOST_ABI_V1_FROZEN",
    "contract": "sley2-host-abi-1",
    "version": 1,
    "digest": digest,
    "permitted_imports": 3,
    "conformance_tests": 37,
}
for key, value in expected.items():
    if frozen.get(key) != value:
        problems.append(f"machine-summary-drift:{key}")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "python3 scripts/check_host_abi_v1.py",
    "python3 scripts/check_host_abi_markers.py",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

report_builder = REPORT_BUILDER.read_text(encoding="utf-8")
for marker in [
    '"host-abi": "python3 scripts/check_host_abi_v1.py"',
    '"host-abi": "codec_and_identity"',
]:
    if marker not in report_builder:
        problems.append(f"conformance-registry-missing:{marker}")

for transcript in [
    "reweave-rw070-ariadne-2026-09-06.log",
    "reweave-rw070-nabu-2026-09-06.log",
    # Passing rounds that close the lanes (failed rounds are preserved
    # alongside, never rewritten).
    "reweave-rw070-ariadne-r5-2026-09-06.log",
    "reweave-rw070-nabu-r4-2026-09-06.log",
]:
    if not (REVIEWS / transcript).is_file():
        problems.append(f"review-missing:{transcript}")

if problems:
    raise SystemExit("\n".join(problems))

print(json.dumps({"contract": "sley2-host-abi-1", "digest": digest, "result": "PASS"}, indent=2, sort_keys=True))

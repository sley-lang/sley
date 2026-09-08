#!/usr/bin/env python3
"""Freeze check for BOOTSTRAP_PROFILE_1 (RW-050 slice 2)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/spec/BOOTSTRAP_PROFILE_1.md"
PROFILE = ROOT / "conformance/bootstrap-profile/v1/profile.json"
ACCEPTED = ROOT / "conformance/bootstrap-profile/v1/accepted.json"
SUMS = ROOT / "conformance/bootstrap-profile/v1/SHA256SUMS"
GENERATOR = ROOT / "scripts/generate_bootstrap_profile_fixtures.py"
MANIFEST = ROOT / "machineresearch/sley-2.0/reweave/bootstrap-manifest.json"
MACHINE_SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
MAKEFILE = ROOT / "Makefile"
REVIEWS = ROOT / "machineresearch/sley-2.0/reviews"

problems: list[str] = []

doc = DOC.read_text(encoding="utf-8")
for marker in [
    "BOOTSTRAP_PROFILE_1",
    "4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630",
    "sley2-bootstrap-profile-1",
    "judge_bootstrap_profile",
    "resolve_bridge_entry",
    "cfg-backedge-loop-form-A",
    "recursion is excluded",
    "default-deny",
    "no `AdapterCall` judgment is",
    "V2B1-over-cap is unreachable by construction",
    "RW-070 ABI/image freeze",
    "rw-050-sufficiency.md",
]:
    if marker not in doc:
        problems.append(f"doc-missing:{marker}")

profile_bytes = PROFILE.read_bytes()
profile = json.loads(profile_bytes.decode("utf-8"))
digest = hashlib.sha256(profile_bytes).hexdigest()
if digest != "4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630":
    problems.append("profile-digest-mismatch")
if digest not in doc:
    problems.append("doc-digest-mismatch")
if profile.get("identity") != "BOOTSTRAP_PROFILE_1" or profile.get("version") != 1:
    problems.append("profile-identity-drift")
if profile.get("contract") != "sley2-bootstrap-profile-1":
    problems.append("profile-contract-drift")
if len(profile.get("permitted_opcodes", [])) != 42:
    problems.append("profile-opcode-count-drift")
if 161 in profile.get("permitted_opcodes", []):
    problems.append("profile-admits-161-by-tag")
if len(profile.get("closure_vectors", {}).get("ids", [])) != 20:
    problems.append("profile-vector-count-drift")
accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
if accepted.get("contract") != "sley2-bootstrap-profile-1":
    problems.append("accepted-contract-drift")
if len(accepted.get("vectors", [])) != 20:
    problems.append("accepted-vector-count-drift")
if profile["closure_vectors"]["accepted_json_sha256"] != hashlib.sha256(
    ACCEPTED.read_bytes()
).hexdigest():
    problems.append("profile-accepted-hash-drift")
sums = SUMS.read_text(encoding="utf-8")
for name, path in [("accepted.json", ACCEPTED), ("profile.json", PROFILE)]:
    if f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {name}\n" not in sums:
        problems.append(f"sums-drift:{name}")

manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
stage_p = manifest.get("stages", {}).get("P", {})
# The manifest binds v1 or its frozen successor (BOOTSTRAP_PROFILE_2, RW-075
# correction), and a successor binding must name v1 as superseded history.
SUCCESSOR = ("BOOTSTRAP_PROFILE_2", "fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459")
if stage_p.get("value") == "BOOTSTRAP_PROFILE_1":
    if stage_p.get("digest") != digest:
        problems.append("manifest-P-digest-drift")
elif stage_p.get("value") == SUCCESSOR[0]:
    if stage_p.get("digest") != SUCCESSOR[1]:
        problems.append("manifest-P-successor-digest-drift")
    provenance = stage_p.get("provenance")
    if not (isinstance(provenance, dict) and "BOOTSTRAP_PROFILE_1" in str(provenance.get("supersedes", ""))):
        problems.append("manifest-P-successor-without-v1-history")
else:
    problems.append("manifest-P-value-drift")
if not isinstance(stage_p.get("provenance"), dict):
    problems.append("manifest-P-provenance-drift")

generator = GENERATOR.read_text(encoding="utf-8")
for marker in [
    "BOOTSTRAP_PROFILE_VECTOR|",
    "profile.json",
    "emit_bootstrap_profile_vectors_for_freeze",
]:
    if marker not in generator:
        problems.append(f"generator-missing:{marker}")

report_builder = (
    ROOT / "scripts/build_independent_conformance_report.py"
).read_text(encoding="utf-8")
for marker in [
    '"bootstrap-profile": "python3 scripts/check_bootstrap_profile_1.py"',
    '"bootstrap-profile": "codec_and_identity"',
]:
    if marker not in report_builder:
        problems.append(f"conformance-registry-missing:{marker}")

summary = json.loads(MACHINE_SUMMARY.read_text(encoding="utf-8"))
frozen = summary.get("bootstrap_profile_1", {})
expected = {
    "status": "BOOTSTRAP_PROFILE_1_FROZEN",
    "contract": "sley2-bootstrap-profile-1",
    "version": 1,
    "digest": digest,
    "permitted_opcodes": 42,
    "closure_vectors": 20,
    "gate": "sley_vm::bootstrap::judge_bootstrap_profile",
}
for key, value in expected.items():
    if frozen.get(key) != value:
        problems.append(f"machine-summary-drift:{key}")

makefile = MAKEFILE.read_text(encoding="utf-8")
for marker in [
    "python3 scripts/check_bootstrap_profile_1.py",
    "python3 scripts/generate_bootstrap_profile_fixtures.py --check",
]:
    if marker not in makefile:
        problems.append(f"makefile-missing:{marker}")

for transcript in [
    "reweave-rw050b2-ariadne-2026-09-06.log",
    "reweave-rw050b2-nabu-2026-09-06.log",
]:
    if not (REVIEWS / transcript).is_file():
        problems.append(f"review-missing:{transcript}")

if problems:
    raise SystemExit("\n".join(problems))

print(
    json.dumps(
        {
            "contract": "sley2-bootstrap-profile-1",
            "digest": digest,
            "result": "PASS",
            "vectors": 20,
        },
        indent=2,
        sort_keys=True,
    )
)

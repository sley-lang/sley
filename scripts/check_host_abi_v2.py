#!/usr/bin/env python3
"""Freeze check for HOST_ABI_V2 (RW-075 correction successor)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/spec/HOST_ABI_V2.md"
RECORD = ROOT / "conformance/host-abi/v2/host-abi.json"
SUMS = ROOT / "conformance/host-abi/v2/SHA256SUMS"
PROFILE = ROOT / "conformance/bootstrap-profile/v2/profile.json"

DIGEST = "bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5"
V1_DIGEST = "e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2"
V2_PROFILE = "fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459"

problems: list[str] = []

doc = DOC.read_text(encoding="utf-8")
for marker in [
    "HOST_ABI_V2",
    DIGEST,
    "sley2-host-abi-2",
    "RHW1",
    "raw-blake3-256",
    V1_DIGEST,
    V2_PROFILE,
    "strict superset",
    "default-deny",
]:
    if marker not in doc:
        problems.append(f"doc-missing:{marker}")

record_bytes = RECORD.read_bytes()
record = json.loads(record_bytes.decode("utf-8"))
digest = hashlib.sha256(record_bytes).hexdigest()
if digest != DIGEST:
    problems.append(f"record-digest-mismatch:{digest}")
if digest not in doc:
    problems.append("doc-digest-mismatch")
if record.get("identity") != "HOST_ABI_V2" or record.get("version") != 2:
    problems.append("record-identity-drift")
if record.get("contract") != "sley2-host-abi-2":
    problems.append("record-contract-drift")

rows = record.get("imports", {}).get("rows", [])
if len(rows) != 4:
    problems.append("imports-row-count-drift")
expected = {
    "B2V1": "534c59312f4252494447452f4232563100000000000000000000000000000000",
    "V2B1": "534c59312f4252494447452f5632423100000000000000000000000000000000",
    "PSH1": "534c59312f4252494447452f5053483100000000000000000000000000000000",
    "RHW1": "534c59312f4252494447452f5248573100000000000000000000000000000000",
}
for row in rows:
    code = row.get("code")
    if row.get("identity_hex") != expected.get(code):
        problems.append(f"imports-identity-drift:{code}")
    if row.get("side_effects") != "none":
        problems.append(f"imports-side-effects-drift:{code}")
if {r.get("code") for r in rows} != {"B2V1", "V2B1", "PSH1", "RHW1"}:
    problems.append("imports-codes-drift")

bindings = record.get("bindings", {})
if bindings.get("bootstrap_profile_digest") != hashlib.sha256(PROFILE.read_bytes()).hexdigest():
    problems.append("bindings-profile-digest-drift")
if bindings.get("bootstrap_profile_digest") != V2_PROFILE:
    problems.append("bindings-profile-v2-mismatch")

sums = SUMS.read_text(encoding="utf-8")
if f"{digest}  host-abi.json\n" not in sums:
    problems.append("sums-drift:host-abi.json")

v1_bytes = (ROOT / "conformance/host-abi/v1/host-abi.json").read_bytes()
if hashlib.sha256(v1_bytes).hexdigest() != V1_DIGEST:
    problems.append("v1-preservation-drift")

if problems:
    raise SystemExit("\n".join(problems))

print(json.dumps({"contract": "sley2-host-abi-2", "digest": digest, "result": "PASS", "imports": 4}, indent=2, sort_keys=True))

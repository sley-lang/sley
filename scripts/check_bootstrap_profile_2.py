#!/usr/bin/env python3
"""Freeze check for BOOTSTRAP_PROFILE_2 (RW-075 correction successor)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/spec/BOOTSTRAP_PROFILE_2.md"
PROFILE = ROOT / "conformance/bootstrap-profile/v2/profile.json"
ACCEPTED = ROOT / "conformance/bootstrap-profile/v2/accepted.json"
SUMS = ROOT / "conformance/bootstrap-profile/v2/SHA256SUMS"

DIGEST = "fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459"
V1_DIGEST = "4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630"

problems: list[str] = []

doc = DOC.read_text(encoding="utf-8")
for marker in [
    "BOOTSTRAP_PROFILE_2",
    DIGEST,
    "sley2-bootstrap-profile-2",
    "RHW1",
    "raw-blake3-256",
    V1_DIGEST,
    "strict superset",
    "default-deny",
]:
    if marker not in doc:
        problems.append(f"doc-missing:{marker}")

profile_bytes = PROFILE.read_bytes()
profile = json.loads(profile_bytes.decode("utf-8"))
digest = hashlib.sha256(profile_bytes).hexdigest()
if digest != DIGEST:
    problems.append(f"profile-digest-mismatch:{digest}")
if digest not in doc:
    problems.append("doc-digest-mismatch")
if profile.get("identity") != "BOOTSTRAP_PROFILE_2" or profile.get("version") != 2:
    problems.append("profile-identity-drift")
if profile.get("contract") != "sley2-bootstrap-profile-2":
    problems.append("profile-contract-drift")
if len(profile.get("permitted_opcodes", [])) != 42:
    problems.append("profile-opcode-count-drift")
if 161 in profile.get("permitted_opcodes", []):
    problems.append("profile-admits-161-by-tag")
rows = profile.get("permitted_imports", {}).get("rows", [])
if {r.get("code") for r in rows} != {"B2V1", "V2B1", "PSH1", "RHW1"}:
    problems.append("profile-import-codes-drift")
if len(rows) != 4:
    problems.append("profile-import-count-drift")
sup = profile.get("supersedes", {})
if sup.get("identity") != "BOOTSTRAP_PROFILE_1" or sup.get("digest") != V1_DIGEST:
    problems.append("profile-supersession-drift")
if len(profile.get("closure_vectors", {}).get("ids", [])) != 20:
    problems.append("profile-vector-count-drift")
accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
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

# V1 preserved byte-identical
v1_bytes = (ROOT / "conformance/bootstrap-profile/v1/profile.json").read_bytes()
if hashlib.sha256(v1_bytes).hexdigest() != V1_DIGEST:
    problems.append("v1-preservation-drift")

if problems:
    raise SystemExit("\n".join(problems))

print(json.dumps({"contract": "sley2-bootstrap-profile-2", "digest": digest, "result": "PASS", "vectors": 20, "imports": 4}, indent=2, sort_keys=True))

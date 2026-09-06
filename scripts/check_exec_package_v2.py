#!/usr/bin/env python3
"""Freeze check for EXEC_PACKAGE_V2 (RW-075 correction successor)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/spec/EXEC_PACKAGE_V2.md"
RECORD = ROOT / "conformance/exec-package/v2/exec-package.json"
SUMS = ROOT / "conformance/exec-package/v2/SHA256SUMS"

DIGEST = "f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94"
V1_DIGEST = "9e20da24a3b3647d15d052ce759ed9b1ca7682d950421baf48978ad59a5795d4"
V2_PROFILE = "fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459"

problems: list[str] = []

doc = DOC.read_text(encoding="utf-8")
for marker in [
    "EXEC_PACKAGE_V2",
    DIGEST,
    "sley2-exec-package-2",
    V1_DIGEST,
    V2_PROFILE,
    "package_digests_v2",
    "approve_package_v2",
]:
    if marker not in doc:
        problems.append(f"doc-missing:{marker}")

raw = RECORD.read_bytes()
record = json.loads(raw.decode("utf-8"))
digest = hashlib.sha256(raw).hexdigest()
if digest != DIGEST:
    problems.append(f"record-digest-mismatch:{digest}")
if record.get("identity") != "EXEC_PACKAGE_V2":
    problems.append("record-identity-drift")
if record.get("contract") != "sley2-exec-package-2":
    problems.append("record-contract-drift")
if record.get("version") != 2:
    problems.append("record-version-drift")
frozen = record.get("frozen_references", {})
if frozen.get("bootstrap_profile_digest") != V2_PROFILE:
    problems.append("frozen-profile-drift")
if frozen.get("host_abi_version") != 2:
    problems.append("frozen-abi-version-drift")
sup = record.get("supersedes", {})
if sup.get("identity") != "EXEC_PACKAGE_V1" or sup.get("digest") != V1_DIGEST:
    problems.append("supersession-drift")

sums = SUMS.read_text(encoding="utf-8").strip()
if sums != f"{digest}  exec-package.json":
    problems.append("sums-mismatch")

v1_bytes = (ROOT / "conformance/exec-package/v1/exec-package.json").read_bytes()
if hashlib.sha256(v1_bytes).hexdigest() != V1_DIGEST:
    problems.append("v1-preservation-drift")

if problems:
    raise SystemExit("\n".join(problems))

print(json.dumps({"contract": "sley2-exec-package-2", "digest": digest, "result": "PASS"}, indent=2, sort_keys=True))

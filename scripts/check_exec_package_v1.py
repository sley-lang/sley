#!/usr/bin/env python3
"""Freeze check for EXEC_PACKAGE_V1 and RAW_HASH_V1 (RW-075).

Coverage-mapped and therefore oracle-independent: this script never reads
implementation sources. Production-code marker pins live in
`scripts/check_exec_package_markers.py` (not coverage-mapped), the same split
as the host-ABI checker beside its markers script.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXEC_DOC = ROOT / "docs/spec/EXEC_PACKAGE_V1.md"
EXEC_RECORD = ROOT / "conformance/exec-package/v1/exec-package.json"
EXEC_SUMS = ROOT / "conformance/exec-package/v1/SHA256SUMS"
HASH_DOC = ROOT / "docs/spec/RAW_HASH_V1.md"
HASH_RECORD = ROOT / "conformance/raw-hash/v1/raw-hash.json"
HASH_SUMS = ROOT / "conformance/raw-hash/v1/SHA256SUMS"
HYDRATION_DOC = ROOT / "docs/spec/HOST_HYDRATION_V1.md"

EXEC_DIGEST = "9e20da24a3b3647d15d052ce759ed9b1ca7682d950421baf48978ad59a5795d4"
HASH_DIGEST = "785205fb49490237cbec7ffe2fc4c2b0f98014b9aae76cc54795921e5d969f72"

problems: list[str] = []

for path, digest, identity, contract in [
    (EXEC_RECORD, EXEC_DIGEST, "EXEC_PACKAGE_V1", "sley2-exec-package-1"),
    (HASH_RECORD, HASH_DIGEST, "RAW_BLAKE3_V1", "sley2-raw-hash-1"),
]:
    raw = path.read_bytes()
    record = json.loads(raw.decode("utf-8"))
    actual = hashlib.sha256(raw).hexdigest()
    if actual != digest:
        problems.append(f"{path.name}-digest-mismatch:{actual}")
    if record.get("identity") != identity:
        problems.append(f"{path.name}-identity-drift")
    if record.get("contract") != contract:
        problems.append(f"{path.name}-contract-drift")
    if record.get("version") != 1:
        problems.append(f"{path.name}-version-drift")

for sums, digest, name in [
    (EXEC_SUMS, EXEC_DIGEST, "exec-package.json"),
    (HASH_SUMS, HASH_DIGEST, "raw-hash.json"),
]:
    text = sums.read_text(encoding="utf-8").strip()
    if text != f"{digest}  {name}":
        problems.append(f"{sums.parent.name}-sums-mismatch")

exec_doc = EXEC_DOC.read_text(encoding="utf-8")
for marker in [
    "EXEC_PACKAGE_V1",
    EXEC_DIGEST,
    "sley2-exec-package-1",
    "SLEYPOBS1",
    "ApprovedExecutionPackage",
    "PACKAGE_RECEIPT_MISMATCH",
    "BOOTSTRAP_PROFILE_1",
    "4f2691504b5c756eae1f5ef01e6e998cc4cd628d4b524b038b10d583bfefd630",
    "e6de00b820a094ec2abc7a6ae43263d7340c2bf1a38a6231e7426fda0f04ecc2",
    "d935d238a4d75d154df128aad630411ca3fdcc18e4db084dcd2317ab73bdb18a",
    "hydrate_verified_definitions",
]:
    if marker not in exec_doc:
        problems.append(f"exec-doc-missing:{marker}")

hash_doc = HASH_DOC.read_text(encoding="utf-8")
for marker in [
    "RAW_BLAKE3_V1",
    HASH_DIGEST,
    "sley2-raw-hash-1",
    "1_048_576",
    "UnknownVariant",
    "charge_action",
]:
    if marker not in hash_doc:
        problems.append(f"hash-doc-missing:{marker}")

hydration_doc = HYDRATION_DOC.read_text(encoding="utf-8")
for marker in [
    "hydrate_verified_definitions",
    "Anything not listed here requires",
    "Semantic (compiler",
    "Structural (host",
]:
    if marker not in hydration_doc:
        problems.append(f"hydration-doc-missing:{marker}")

if problems:
    print("EXEC_PACKAGE_V1_CHECK: FAIL")
    for problem in problems:
        print(f"  - {problem}")
    raise SystemExit(1)
print("EXEC_PACKAGE_V1_CHECK: PASS")

#!/usr/bin/env python3
"""Validate the retained RW-080 partial-component construction manifest."""

from __future__ import annotations

import base64
import copy
import gzip
import hashlib
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = (
    ROOT
    / "machineresearch/sley-2.0/reweave/rw-080-toolchain-component-manifest.json"
)
CONTRACT = "sley2.rw080-toolchain-component-manifest.v1"
EXPECTED_TOKENS = [
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    10,
    11,
    20,
    21,
    22,
    23,
    24,
    25,
    26,
    30,
    31,
    32,
    33,
    40,
    41,
    42,
    43,
    44,
    45,
    46,
    47,
    48,
    49,
    50,
    51,
    60,
    61,
    62,
    63,
    64,
    65,
    66,
    67,
    70,
    71,
    72,
    80,
    81,
]
ENTRY_TOKENS = {
    "driver": 63,
    "codec": 64,
    "checker": 65,
    "lowerer": 66,
    "package_builder": 67,
}
FUNCTION_TOKENS = {
    "driver": 1,
    "codec": 2,
    "checker": 3,
    "lowerer": 4,
    "package_builder": 5,
}


def canonical(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    problems: list[str] = []
    try:
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(json.dumps({"result": "FAIL", "problems": [str(error)]}, indent=2))
        return 1

    if manifest.get("contract") != CONTRACT:
        problems.append("contract")
    if manifest.get("is_canonical_s") is not False:
        problems.append("canonical-s-claim")
    if manifest.get("complete_toolchain_closure") is not False:
        problems.append("complete-closure-claim")
    if any(manifest.get("claims", {}).values()):
        problems.append("premature-stage-claim")

    claimed_digest = manifest.get("manifest_digest")
    digest_input = copy.deepcopy(manifest)
    digest_input.pop("manifest_digest", None)
    if claimed_digest != digest(canonical(digest_input)):
        problems.append("manifest-digest")

    bundle_record = manifest.get("object_bundle", {})
    try:
        bundle = gzip.decompress(base64.b64decode(bundle_record["gzip_base64"], validate=True))
    except (KeyError, ValueError, gzip.BadGzipFile) as error:
        problems.append(f"object-bundle-decode:{error}")
        bundle = b""
    if len(bundle) != bundle_record.get("uncompressed_bytes"):
        problems.append("object-bundle-length")
    if digest(bundle) != bundle_record.get("uncompressed_sha256"):
        problems.append("object-bundle-digest")

    objects = manifest.get("objects", [])
    if [row.get("seed_local_token") for row in objects] != EXPECTED_TOKENS:
        problems.append("object-token-inventory")
    if len({row.get("entity_id") for row in objects}) != len(EXPECTED_TOKENS):
        problems.append("entity-id-uniqueness")
    if len({row.get("object_id") for row in objects}) != len(EXPECTED_TOKENS):
        problems.append("object-id-uniqueness")
    cursor = 0
    by_token: dict[int, dict] = {}
    for row in objects:
        token = row.get("seed_local_token")
        if isinstance(token, int):
            by_token[token] = row
        if row.get("identity_origin") != "derived":
            problems.append(f"object-{token}-identity-origin")
        if row.get("construction_method") != "seed-assembled":
            problems.append(f"object-{token}-construction-method")
        if row.get("bundle_offset") != cursor:
            problems.append(f"object-{token}-offset")
            continue
        length = row.get("stored_bytes_length")
        if not isinstance(length, int) or length < 32:
            problems.append(f"object-{token}-length")
            continue
        stored = bundle[cursor : cursor + length]
        cursor += length
        if len(stored) != length:
            problems.append(f"object-{token}-truncated")
            continue
        if digest(stored) != row.get("stored_bytes_sha256"):
            problems.append(f"object-{token}-digest")
        try:
            object_id = bytes.fromhex(row["object_id"])
            entity_id = bytes.fromhex(row["entity_id"])
        except (KeyError, ValueError):
            problems.append(f"object-{token}-id-encoding")
            continue
        if len(object_id) != 32 or stored[-32:] != object_id:
            problems.append(f"object-{token}-trailer")
        if len(entity_id) != 32:
            problems.append(f"object-{token}-entity-width")
    if cursor != len(bundle):
        problems.append("object-bundle-trailing")

    entries = manifest.get("entry_points", {})
    for name, token in ENTRY_TOKENS.items():
        if entries.get(name) != by_token.get(token, {}).get("entity_id"):
            problems.append(f"entry-{name}")
    functions = manifest.get("entry_functions", {})
    for name, token in FUNCTION_TOKENS.items():
        if functions.get(name) != by_token.get(token, {}).get("entity_id"):
            problems.append(f"entry-function-{name}")

    driver = manifest.get("driver_contract", {})
    for field, token in {
        "build_manifest_type": 70,
        "built_toolchain_type": 71,
        "build_error_type": 72,
    }.items():
        if driver.get(field) != by_token.get(token, {}).get("entity_id"):
            problems.append(f"driver-{field}")
    if driver.get("scaffold_result") != "typed BuildError::Incomplete":
        problems.append("driver-scaffold-result")

    anchors = manifest.get("anchors", {})
    for name, token in {"contract_root": 80, "test_root": 81}.items():
        anchor = anchors.get(name, {})
        object_row = by_token.get(token, {})
        if anchor.get("seed_local_token") != token:
            problems.append(f"{name}-token")
        if anchor.get("entity_id") != object_row.get("entity_id"):
            problems.append(f"{name}-entity")
        if anchor.get("object_id") != object_row.get("object_id"):
            problems.append(f"{name}-object")
        if anchor.get("stored_bytes_sha256") != object_row.get("stored_bytes_sha256"):
            problems.append(f"{name}-digest")

    state = manifest.get("state_root", {})
    try:
        root_bytes = gzip.decompress(
            base64.b64decode(state["stored_bytes_gzip_base64"], validate=True)
        )
        root_id = bytes.fromhex(state["root"])
    except (KeyError, ValueError, gzip.BadGzipFile) as error:
        problems.append(f"state-root-decode:{error}")
        root_bytes = b""
        root_id = b""
    if len(root_bytes) != state.get("stored_bytes_uncompressed_length"):
        problems.append("state-root-length")
    if digest(root_bytes) != state.get("stored_bytes_sha256"):
        problems.append("state-root-digest")
    if len(root_id) != 32 or root_bytes[-32:] != root_id:
        problems.append("state-root-trailer")
    if state.get("entity_binding_count") != len(EXPECTED_TOKENS):
        problems.append("state-root-binding-count")
    if state.get("entry_point_count") != len(ENTRY_TOKENS):
        problems.append("state-root-entry-count")
    if state.get("dependency_root_count") != 0:
        problems.append("state-root-dependency-count")

    policy = manifest.get("anchors", {}).get("policy_root", {})
    try:
        policy_bytes = gzip.decompress(
            base64.b64decode(policy["stored_bytes_gzip_base64"], validate=True)
        )
        policy_id = bytes.fromhex(policy["policy_root_id"])
    except (KeyError, ValueError, gzip.BadGzipFile) as error:
        problems.append(f"policy-root-decode:{error}")
        policy_bytes = b""
        policy_id = b""
    if len(policy_bytes) != policy.get("stored_bytes_uncompressed_length"):
        problems.append("policy-root-length")
    if digest(policy_bytes) != policy.get("stored_bytes_sha256"):
        problems.append("policy-root-digest")
    if len(policy_id) != 32 or policy_bytes[-32:] != policy_id:
        problems.append("policy-root-trailer")

    result = {
        "contract": CONTRACT,
        "objects": len(objects),
        "object_bytes": len(bundle),
        "root_bytes": len(root_bytes),
        "policy_bytes": len(policy_bytes),
        "state_root": state.get("root"),
        "result": "PASS" if not problems else "FAIL",
        "problems": problems,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Mechanical dependency and rule audit for the S20-430 thin CLI (contract section 5)."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "crates/sley-cli"
MANIFEST = CRATE / "Cargo.toml"
ALLOWED_DEPENDENCIES = {"sley-protocol", "sley-json-bridge", "serde_json"}
ALLOWED_DEV_DEPENDENCIES = {"sley-repo", "sley-id", "sley-scb1", "sley-protocol", "sley-json-bridge", "serde_json"}
KERNEL_CRATES = (
    "sley_ssmc",
    "sley_check",
    "sley_query",
    "sley_mutate",
    "sley_txn",
    "sley_repo",
    "sley_policy",
    "sley_vm",
    "sley_state_root",
    "sley_store",
    "sley_adapter",
    "sley_schema",
    "sley_scb1",
)
JUDGMENT_PATTERNS = (
    re.compile(r"\bfn validate"),
    re.compile(r"\bfn judge"),
    re.compile(r"\bfn check_"),
    re.compile(r"Method::[A-Z][A-Za-z]+\s*(\||=>)"),
    re.compile(r"\b(100|101|102|103|104|20\d|21[0-4]|30[0-5]|40[0-4]|50[0-4]|60[0-4])\s*=>"),
)
METHOD_NAME = re.compile(r'"(session\.|workspace\.|refs\.|revision\.|branch\.|merge\.|exchange\.|gc\.|query\.|candidate\.|receipt\.|ref\.|tests\.)[a-z._]*"')


def section(manifest: str, name: str) -> set[str]:
    match = re.search(rf"^\[{re.escape(name)}\]\n(.*?)(?=^\[|\Z)", manifest, re.S | re.M)
    if not match:
        return set()
    return {line.split("=", 1)[0].strip() for line in match.group(1).splitlines() if "=" in line and not line.startswith("#")}


def main() -> int:
    if not CRATE.exists():
        print(json.dumps({"contract": "s20-430-cli-rule-audit-v1", "crate": "absent", "result": "PASS"}, sort_keys=True))
        return 0
    problems: list[str] = []
    manifest = MANIFEST.read_text(encoding="utf-8")
    dependencies = section(manifest, "dependencies")
    if not dependencies <= ALLOWED_DEPENDENCIES:
        problems.append(f"dependencies:{sorted(dependencies - ALLOWED_DEPENDENCIES)}")
    dev_dependencies = section(manifest, "dev-dependencies")
    if not dev_dependencies <= ALLOWED_DEV_DEPENDENCIES:
        problems.append(f"dev-dependencies:{sorted(dev_dependencies - ALLOWED_DEV_DEPENDENCIES)}")
    for other in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        if other.parent.name != "sley-cli" and "sley-cli" in other.read_text(encoding="utf-8"):
            problems.append(f"reverse-dependency:{other.parent.name}")

    sources = sorted(CRATE.glob("src/**/*.rs"))
    if not sources:
        problems.append("no-sources")
    frame_literals = 0
    encode_calls = 0
    for path in sources:
        text = path.read_text(encoding="utf-8")
        # Tests live under `mod tests` or a `tests` directory and may use fixtures.
        production = text.split("#[cfg(test)]", 1)[0]
        for crate in KERNEL_CRATES:
            if re.search(rf"\b{crate}\b", production):
                problems.append(f"kernel-crate:{path.name}:{crate}")
        if "ProtocolFailure {" in production:
            problems.append(f"failure-literal:{path.name}")
        frame_literals += production.count("ProtocolFrame {")
        encode_calls += len(re.findall(r"\bencode_frame\(", production))
        for pattern in JUDGMENT_PATTERNS:
            if pattern.search(production):
                problems.append(f"judgment:{path.name}:{pattern.pattern}")
        if METHOD_NAME.search(production):
            problems.append(f"method-name:{path.name}")
        # The offer carries no transport feature (contract section 2): the
        # endpoint must never name the JSON bridge feature bit.
        if "FEATURE_JSON_BRIDGE" in production:
            problems.append(f"transport-feature:{path.name}")
    if frame_literals > 1:
        problems.append(f"frame-literals:{frame_literals}")
    if encode_calls > 1:
        problems.append(f"encode-frame-calls:{encode_calls}")

    result = {
        "contract": "s20-430-cli-rule-audit-v1",
        "crate": "present",
        "dependencies": sorted(dependencies),
        "frame_literals": frame_literals,
        "encode_frame_calls": encode_calls,
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

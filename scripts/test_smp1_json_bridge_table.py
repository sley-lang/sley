#!/usr/bin/env python3
"""Bounded negative checks for the S20-420 method-table generator.

Covers missing/duplicate/out-of-section rows and default version-1
leakage. Plain script, no test framework: any failure prints FAIL and
exits nonzero.
"""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC_TEXT = (ROOT / "docs/spec/SMP1.md").read_text(encoding="utf-8")


def load_generator():
    path = ROOT / "scripts/generate_smp1_json_bridge_table.py"
    spec = importlib.util.spec_from_file_location("bridge_table", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules["bridge_table"] = module
    spec.loader.exec_module(module)
    return module


def expect_fail(label: str, func, *args) -> None:
    try:
        func(*args)
    except SystemExit:
        print(json.dumps({"case": label, "result": "PASS"}))
        return
    print(json.dumps({"case": label, "result": "FAIL", "problem": "accepted bad input"}))
    raise SystemExit(1)


def main() -> int:
    gen = load_generator()
    spec = SPEC_TEXT
    v1_row = "| 100 | `session.open` | `ProtocolHandshakeId` | `SessionId` | S20-330 |"

    # The default build is the frozen version 1 table exactly: no v2 leakage.
    default = gen.build()
    if default["method_count"] != 41:
        print(json.dumps({"case": "default-count", "result": "FAIL"}))
        return 1
    if any(method["tag"] in (306, 307) for method in default["methods"]):
        print(json.dumps({"case": "default-v1-leakage", "result": "FAIL"}))
        return 1
    frozen = (ROOT / "conformance/smp1-json-bridge/v1/methods.json").read_text(encoding="utf-8")
    if gen.render(default) != frozen:
        print(json.dumps({"case": "default-frozen-bytes", "result": "FAIL"}))
        return 1
    print(json.dumps({"case": "default-v1-exact", "result": "PASS"}))

    explicit = gen.build(2)
    if explicit["method_count"] != 43 or explicit["reserved_count"] != 4:
        print(json.dumps({"case": "explicit-v2-counts", "result": "FAIL"}))
        return 1
    print(json.dumps({"case": "explicit-v2-counts", "result": "PASS"}))

    # Missing sections fail.
    expect_fail(
        "missing-v1-section",
        gen.parse_tables,
        spec.replace("### Protocol version 1\n", ""),
    )
    expect_fail(
        "missing-v2-section",
        gen.parse_tables,
        spec.replace("### Protocol version 2 additions\n", ""),
    )
    # Duplicate sections fail.
    expect_fail(
        "duplicate-v2-section",
        gen.parse_tables,
        spec.replace(
            "### Protocol version 2 additions\n",
            "### Protocol version 2 additions\n### Protocol version 2 additions\n",
            1,
        ),
    )
    # A method row outside the declared sections fails.
    expect_fail(
        "out-of-section-row",
        gen.parse_tables,
        spec + "\n" + v1_row + "\n",
    )
    # A duplicate name inside the v1 section fails.
    expect_fail(
        "duplicate-name",
        gen.parse_tables,
        spec.replace(
            "| 306 | `entity.version` |",
            "| 306 | `session.open` |",
            1,
        ),
    )
    # A wrong v2 tag fails.
    expect_fail(
        "wrong-v2-tag",
        gen.parse_tables,
        spec.replace("| 306 | `entity.version` |", "| 308 | `entity.version` |", 1),
    )
    # A dropped v1 row fails the count.
    expect_fail(
        "missing-v1-row",
        gen.parse_tables,
        spec.replace(v1_row + "\n", "", 1),
    )
    print(json.dumps({"cases": 9, "result": "PASS"}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

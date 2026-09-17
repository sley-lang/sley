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

    # The explicit version 3 table overrides the frozen reserved
    # 601/602 rows and unions the three fresh native rows, all five live,
    # with the native provenance in the header; the frozen v3 bytes on
    # disk match the render exactly.
    native_spec = (ROOT / "docs/spec/NATIVE_TEST_ADMISSION_V1.md").read_text(encoding="utf-8")
    v3 = gen.build(3)
    if v3["method_count"] != 46 or v3["reserved_count"] != 2:
        print(json.dumps({"case": "explicit-v3-counts", "result": "FAIL"}))
        return 1
    v3_tags = [method["tag"] for method in v3["methods"]]
    v2_tags = [method["tag"] for method in explicit["methods"]]
    if v3_tags != sorted(set(v2_tags) | {605, 606, 607}):
        print(json.dumps({"case": "explicit-v3-union", "result": "FAIL"}))
        return 1
    if [tag for tag in v3_tags if tag in (605, 606, 607)] != [605, 606, 607]:
        print(json.dumps({"case": "explicit-v3-additions", "result": "FAIL"}))
        return 1
    v3_live = {
        method["tag"]: method["reserved"]
        for method in v3["methods"]
        if method["tag"] in (601, 602, 605, 606, 607)
    }
    if (
        v3_live.get(601)
        or v3_live.get(602)
        or v3_live.get(605)
        or v3_live.get(606)
        or v3_live.get(607)
    ):
        print(json.dumps({"case": "explicit-v3-live-rows", "result": "FAIL"}))
        return 1
    v3_reserved = sorted(
        method["tag"] for method in v3["methods"] if method["reserved"]
    )
    if v3_reserved != [305, 503]:
        print(json.dumps({"case": "explicit-v3-reserved-rows", "result": "FAIL"}))
        return 1
    if v3.get("v3_source") != "docs/spec/NATIVE_TEST_ADMISSION_V1.md":
        print(json.dumps({"case": "explicit-v3-source", "result": "FAIL"}))
        return 1
    frozen_v3 = (ROOT / "conformance/smp1-json-bridge/v3/methods.json").read_text(encoding="utf-8")
    if gen.render(v3) != frozen_v3:
        print(json.dumps({"case": "explicit-v3-frozen-bytes", "result": "FAIL"}))
        return 1
    print(json.dumps({"case": "explicit-v3-counts", "result": "PASS"}))

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
    # A duplicate name inside the v1 section fails via the v1 uniqueness
    # guard: the 604 row takes another live v1 name while tags, counts,
    # row order, and both exact v2 additions stay valid.
    expect_fail(
        "duplicate-name",
        gen.parse_tables,
        spec.replace(
            "| 604 | `report` |",
            "| 604 | `cancel` |",
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
    # A missing native v3 section fails.
    expect_fail(
        "missing-v3-section",
        gen.parse_v3_additions,
        native_spec.replace(
            "## Appendix D. SMP v3 additions table (machine-readable, revision 5)\n", ""
        ),
    )
    # A method row outside the native v3 section fails.
    expect_fail(
        "out-of-v3-section-row",
        gen.parse_v3_additions,
        native_spec + "\n" + v1_row + "\n",
    )
    # A wrong v3 tag fails.
    expect_fail(
        "wrong-v3-tag",
        gen.parse_v3_additions,
        native_spec.replace("| 605 | `tests.report_read` |", "| 608 | `tests.report_read` |", 1),
    )
    # A live v3 row naming the wrong records fails: 606 must name its
    # Appendix C replay records exactly.
    expect_fail(
        "misnamed-606-row",
        gen.parse_v3_additions,
        native_spec.replace(
            "| 606 | `tests.replay` | `tests.replay.request` | `tests.replay.response` | S20-620 |",
            "| 606 | `tests.replay` | `replay token` | `replay page` | S20-620 |",
            1,
        ),
    )
    # A reserved-back 607 row fails: replay and status are live since N7d-2.
    expect_fail(
        "relocked-607-row",
        gen.parse_v3_additions,
        native_spec.replace(
            "| 607 | `tests.attempt_status` | `tests.attempt_status.request` | `tests.attempt_status.response` | S20-620 |",
            "| 607 | `tests.attempt_status` | reserved | reserved | S20-620 |",
            1,
        ),
    )
    # A reserved-back 601 row fails: the selection reads are live since N7c.
    expect_fail(
        "relocked-v3-row",
        gen.parse_v3_additions,
        native_spec.replace(
            "| 601 | `tests.selected` | `tests.selected.request` | `tests.selected.response` | S20-620 |",
            "| 601 | `tests.selected` | reserved | reserved | S20-620 |",
            1,
        ),
    )
    # A live 601 row naming the wrong records fails.
    expect_fail(
        "misnamed-v3-row",
        gen.parse_v3_additions,
        native_spec.replace(
            "| 601 | `tests.selected` | `tests.selected.request` | `tests.selected.response` | S20-620 |",
            "| 601 | `tests.selected` | `tests picked` | `tests picked back` | S20-620 |",
            1,
        ),
    )
    # A live 605 row naming the wrong records fails.
    expect_fail(
        "misnamed-605-row",
        gen.parse_v3_additions,
        native_spec.replace(
            "| 605 | `tests.report_read` | `tests.report_read.request` | `tests.report_read.response` | S20-620 |",
            "| 605 | `tests.report_read` | `report token` | `report page` | S20-620 |",
            1,
        ),
    )
    print(json.dumps({"cases": 17, "result": "PASS"}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

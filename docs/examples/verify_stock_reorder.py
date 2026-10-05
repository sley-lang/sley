#!/usr/bin/env python3
"""Exercise the packaged application, host boundary and an independent oracle."""

from __future__ import annotations

import argparse
import csv
import io
import json
import random
from pathlib import Path
import shutil
import subprocess

import stock_reorder as app


def oracle(values: list) -> dict:
    """Independent integer specification, used only by this verifier."""
    hand, ordered, reserved, demand, days, safety, pack = values
    if min(values) < 0 or pack == 0:
        return {"Err": "InvalidInput"}
    projected = demand * days
    buffered = projected + safety
    target = buffered + reserved
    available = hand + ordered
    if max(projected, buffered, target, available) > app.I64_MAX:
        return {"Err": "Overflow"}
    gap = max(0, target - available)
    result = ((gap + pack - 1) // pack) * pack
    return {"Err": "Overflow"} if result > app.I64_MAX else {"Ok": result}


def verify(agent: Path, output: Path) -> dict:
    output.mkdir(parents=True, exist_ok=False)
    built = app.build(agent, output / "build")
    package = output / "build/package"
    rng = random.Random(206)
    inputs = [[rng.randrange(101) for _ in range(7)] for _ in range(400)]
    for position in range(7):
        for boundary in [-2**63, -1, 0, 1, app.I64_MAX - 1, app.I64_MAX]:
            row = [0, 0, 0, 1, 1, 0, 1]
            row[position] = boundary
            inputs.append(row)
    inputs += [[0, 0, 0, app.I64_MAX, 2, 0, 1],
               [0, 0, 0, 1, app.I64_MAX, 0, 2],
               [app.I64_MAX, 1, 0, 0, 0, 0, 1]]
    rows = [(f"case-{index}", row) for index, row in enumerate(inputs)]
    work = output / "oracle"
    work.mkdir()
    _, actual = app.calculate(agent, package, rows, work)
    expected = [oracle(row) for row in inputs]
    app.require(actual == expected, "Sley results differ from the independent integer specification")
    app.write_json(output / "oracle-results.json", {"inputs": inputs, "expected": expected, "actual": actual})

    # Test the distributed entrypoint after extracting the archive, without a source checkout.
    extracted = output / "unpacked"
    shutil.unpack_archive(str(output / "build/stock-reorder.zip"), str(extracted))
    inventory = output / "inventory.csv"
    sample = io.StringIO(newline="")
    writer = csv.writer(sample, lineterminator="\n")
    writer.writerow(["sku", *app.FIELDS])
    samples = [[10, 5, 3, 4, 7, 2, 6], [100, 0, 0, 4, 7, 2, 6], [0, 0, 0, 1, 1, 0, 0]]
    for index, row in enumerate(samples):
        writer.writerow([f"SKU-{index}", *row])
    inventory.write_text(sample.getvalue(), encoding="utf-8")
    receipt = output / "report-receipt.json"
    command = ["python3", str(extracted / "stock-reorder/stock_reorder.py"), "--agent", str(agent),
               "report", "--package", str(extracted / "stock-reorder"), "--inventory", str(inventory),
               "--receipt", str(receipt)]
    reported = subprocess.run(command, capture_output=True, text=True, check=False, timeout=120)
    app.require(reported.returncode == 1, "mixed business results should return exit 1")
    app.require(reported.stdout == "sku,reorder_units,status\nSKU-0,18,ok\nSKU-1,0,ok\nSKU-2,,InvalidInput\n",
                f"unexpected CSV report: {reported.stdout} {reported.stderr}")
    app.require(json.loads(receipt.read_text())["inventory_sha256"] == app.sha(inventory.read_bytes()),
                "receipt must bind the input bytes")
    (output / "report.csv").write_text(reported.stdout, encoding="utf-8")
    preserved = receipt.read_bytes()
    duplicate = subprocess.run(command, capture_output=True, text=True, check=False, timeout=120)
    app.require(duplicate.returncode == 2 and duplicate.stdout == "" and receipt.read_bytes() == preserved,
                "existing receipt must refuse without output or replacement")

    # Reject malformed host input before invoking Sley or emitting a partial report.
    header = "sku," + ",".join(app.FIELDS) + "\n"
    valid = "OK,0,0,0,1,1,0,1\n"
    malformed = {
        "duplicate": header + valid + valid,
        "extra-column": header + "OK,0,0,0,1,1,0,1,2\n",
        "missing-column": header + "OK,0,0,0,1,1,0\n",
        "formula-sku": header + "=1+1,0,0,0,1,1,0,1\n",
        "decimal": header + "OK,0.5,0,0,1,1,0,1\n",
        "out-of-i64": header + f"OK,{2**63},0,0,1,1,0,1\n",
        "empty": header,
        "bad-header": "sku,count\nOK,1\n",
        "unclosed-quote": header + '"OK,0,0,0,1,1,0,1\n',
        "row-limit": header + "".join(f"S{i},0,0,0,1,1,0,1\n" for i in range(10001)),
        "byte-limit": "x" * 2_000_001,
    }
    rejected = []
    for name, text in malformed.items():
        path = output / f"invalid-{name}.csv"
        path.write_text(text, encoding="utf-8")
        try:
            app.read_inventory(path)
        except (ValueError, csv.Error):
            rejected.append(name)
        else:
            raise ValueError(f"accepted malformed CSV: {name}")

    corrupted = output / "corrupted-package"
    shutil.copytree(package, corrupted)
    (corrupted / "candidate.hex").write_text("00\n", encoding="utf-8")
    try:
        app.load_package(corrupted)
    except ValueError:
        corruption_refused = True
    else:
        raise ValueError("accepted changed package bytes")
    files = {str(p.relative_to(output / "build")): app.sha(p.read_bytes())
             for p in (output / "build").rglob("*") if p.is_file()}
    try:
        app.build(agent, output / "build")
    except FileExistsError:
        pass
    else:
        raise ValueError("existing build output was reused")
    app.require(files == {str(p.relative_to(output / "build")): app.sha(p.read_bytes())
                          for p in (output / "build").rglob("*") if p.is_file()},
                "existing output refusal changed files")

    # Maintenance adds an explicit regression case in a new version; v1 is preserved.
    maintained = json.loads((package / "frame.json").read_text())
    maintained["test_tables"][0]["cases"].append(
        {"name": "reserved_stock_regression", "args": [8, 0, 7, 1, 7, 0, 4], "expect": {"Ok": 8}})
    revised_frame = output / "maintenance-frame.json"
    app.write_json(revised_frame, maintained)
    version2 = app.build(agent, output / "maintenance", revised_frame)
    app.require(version2["manifest"]["base_root"] == built["manifest"]["base_root"], "maintenance base differs")
    app.require(version2["manifest"]["candidate_stored_sha256"] != built["manifest"]["candidate_stored_sha256"] and
                version2["manifest"]["proposed_root"] != built["manifest"]["proposed_root"],
                "new regression case must produce a separately reviewed artifact")
    app.require(files == {str(p.relative_to(output / "build")): app.sha(p.read_bytes())
                          for p in (output / "build").rglob("*") if p.is_file()}, "maintenance changed v1")
    # A wrong expected result must prevent packaging, with its failed trial preserved.
    broken = json.loads(json.dumps(maintained))
    broken["test_tables"][0]["cases"][-1]["expect"] = {"Ok": 4}
    broken_frame = output / "wrong-expectation-frame.json"
    app.write_json(broken_frame, broken)
    try:
        app.build(agent, output / "failed-maintenance", broken_frame)
    except ValueError:
        app.require(not (output / "failed-maintenance/stock-reorder.zip").exists(), "failed test produced a package")
        app.require(json.loads((output / "failed-maintenance/result.json").read_text())["result"] == "FAIL",
                    "failed test result was not preserved")
    else:
        raise ValueError("packaged a failing maintenance candidate")
    result = {"result": "PASS", "agent_sha256": app.AGENT_SHA256,
              "application_script_sha256": app.sha(Path(app.__file__).read_bytes()),
              "verifier_script_sha256": app.sha(Path(__file__).read_bytes()),
              "canonical_tests": built["manifest"]["selected_tests"],
              "independent_oracle_cases": len(inputs), "malformed_csv_refusals": rejected,
              "archive_entrypoint_and_csv": "PASS", "package_corruption_refused": corruption_refused,
              "existing_receipt_and_build_preserved": True,
              "maintenance_canonical_tests": version2["manifest"]["selected_tests"],
              "maintenance_changed_root_and_bytes": True, "original_package_preserved": True,
              "failing_maintenance_not_packaged": True, "tested_candidate_committed": False}
    app.write_json(output / "verification.json", result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(app.checked_agent(args.agent), args.output.absolute()), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

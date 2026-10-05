#!/usr/bin/env python3
"""Package and run an advisory Sley stock-reorder calculation over local CSV.

Python owns CSV, files, subprocesses and packaging. Sley owns the reorder rule.
No network, native supervisor, purchase order or authoritative commit is used.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

AGENT_SHA256 = "7737c4f09c7236555155c25a967b4d52a8fb8f815464e7bcb33935e12e2728c5"
FIELDS = ["on_hand", "on_order", "reserved", "daily_demand", "days", "safety", "pack"]
I64_MAX = 2**63 - 1
PACKAGE_FILES = {"base.pack", "candidate.hex", "names.json", "frame.json", "view.txt", "stock_reorder.py"}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def checked_agent(path: Path) -> Path:
    path = path.resolve(strict=True)
    require(sha(path.read_bytes()) == AGENT_SHA256, "expected the published 2.0.6 agent SHA-256")
    return path


def run(agent: Path, workspace: Path, *args: str, expected: int = 0,
        events: list | None = None, lines: bool = False) -> object:
    portable = []
    for arg in args:
        try:
            portable.append(str(Path(arg).relative_to(workspace.parent)) if Path(arg).is_absolute() else arg)
        except ValueError:
            portable.append(arg)
    command = [str(agent), "--workspace", workspace.name, "--json", *portable]
    result = subprocess.run(command, cwd=workspace.parent, capture_output=True, text=True, timeout=60,
                            env={"PATH": os.defpath, "LANG": "C.UTF-8"}, check=False)
    if events is not None:
        events.append({"workspace": workspace.name, "args": portable, "exit_code": result.returncode,
                       "stdout": result.stdout, "stderr": result.stderr})
    require(result.returncode == expected,
            f"{args[0]}: expected exit {expected}, got {result.returncode}: {result.stdout} {result.stderr}")
    return [json.loads(line) for line in result.stdout.splitlines()] if lines else json.loads(result.stdout)


def application_frame() -> dict:
    error = lambda name: {"Err": name}
    cases = [
        ([10, 5, 3, 4, 7, 2, 6], {"Ok": 18}),
        ([100, 0, 0, 4, 7, 2, 6], {"Ok": 0}),
        ([0, 0, 0, 0, 0, 0, 1], {"Ok": 0}),
        ([0, 0, 0, 1, 6, 0, 6], {"Ok": 6}),
        ([0, 0, 0, 1, 7, 0, 6], {"Ok": 12}),
        ([0, 20, 5, 2, 10, 0, 4], {"Ok": 8}),
        ([0, 0, 0, 1, 1, 0, 0], error("InvalidInput")),
        ([0, 0, 0, I64_MAX, 2, 0, 1], error("Overflow")),
        ([I64_MAX, 1, 0, 0, 0, 0, 1], error("Overflow")),
        ([0, 0, 0, 1, I64_MAX, 0, 2], error("Overflow")),
    ]
    for index in range(len(FIELDS)):
        args = [0, 0, 0, 1, 1, 0, 1]
        args[index] = -1
        cases.append((args, error("InvalidInput")))
    guards = [["!InvalidInput", "if", ["lt", field, 0]] for field in FIELDS]
    guards.append(["!InvalidInput", "if", ["eq", "pack", 0]])
    return {"af1": 1, "afx": 1,
            "types": [{"name": "ReorderError", "variant": ["InvalidInput", "Overflow"]}],
            "fns": [
                {"fn": "round_pack", "params": [["needed", "i64"], ["pack", "i64"]],
                 "returns": "Result<i64,ReorderError>", "blocks": [
                     {"name": "entry", "ops": [
                         ["!InvalidInput", "if", ["lt", "needed", 0]],
                         ["!InvalidInput", "if", ["le", "pack", 0]],
                         ["q", "div?Overflow", "needed", "pack"],
                         ["r", "rem?Overflow", "needed", "pack"]],
                      "term": ["cond", ["eq", "r", 0], ["exact", "needed"], ["up", "q"]]},
                     {"name": "exact", "params": [["value", "i64"]], "term": ["ok", "value"]},
                     {"name": "up", "params": [["q", "i64"]], "ops": [
                         ["more", "add?Overflow", "q", 1], ["units", "mul?Overflow", "more", "pack"]],
                      "term": ["ok", "units"]}]},
                {"fn": "reorder", "params": [[field, "i64"] for field in FIELDS],
                 "returns": "Result<i64,ReorderError>", "blocks": [
                     {"name": "entry", "ops": guards + [
                         ["demand", "mul?Overflow", "daily_demand", "days"],
                         ["buffered", "add?Overflow", "demand", "safety"],
                         ["target", "add?Overflow", "buffered", "reserved"],
                         ["available", "add?Overflow", "on_hand", "on_order"]],
                      "term": ["cond", ["ge", "available", "target"], ["enough"],
                               ["short", "target", "available"]]},
                     {"name": "enough", "term": ["ok", 0]},
                     {"name": "short", "params": [["target", "i64"], ["available", "i64"]],
                      "ops": [["needed", "sub?Overflow", "target", "available"],
                              ["rounded", "call?", "round_pack", "needed", "pack"]],
                      "term": ["ok", "rounded"]}]}],
            "test_tables": [{"name": "reorder_cases", "fn": "reorder", "cases": [
                {"name": f"reorder_case_{index}", "args": args, "expect": expected}
                for index, (args, expected) in enumerate(cases)]}]}


def repository_files(workspace: Path) -> dict:
    repo = workspace / "repo"
    return {str(p.relative_to(repo)): sha(p.read_bytes()) for p in repo.rglob("*") if p.is_file()}


def load_package(package: Path) -> dict:
    manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
    require(manifest["format"] == "sley-stock-reorder-v1", "unknown package format")
    require(manifest["agent_sha256"] == AGENT_SHA256, "package agent identity differs")
    require(set(manifest["files"]) == PACKAGE_FILES, "package file inventory differs")
    for name, digest in manifest["files"].items():
        require(sha((package / name).read_bytes()) == digest, f"package file changed: {name}")
    require(sha(bytes.fromhex((package / "candidate.hex").read_text().strip())) ==
            manifest["candidate_stored_sha256"], "candidate digest differs")
    require(manifest["tested_candidate_committed"] is False, "unexpected admission claim")
    return manifest


def replay(agent: Path, package: Path, workspace: Path, events: list | None = None) -> tuple:
    manifest = load_package(package)
    workspace.mkdir()
    shutil.copyfile(package / "base.pack", workspace / "base.pack")
    shutil.copyfile(package / "names.json", workspace / "names.json")
    require(run(agent, workspace, "view", events=events)["root"] == manifest["base_root"],
            "package base root differs")
    before = repository_files(workspace)
    submitted = run(agent, workspace, "submit", str(package / "candidate.hex"), events=events)["submitted"]
    require((workspace / "final_candidate.hex").read_bytes() == (package / "candidate.hex").read_bytes(),
            "submission changed candidate bytes")
    require(run(agent, workspace, "view", "--after", submitted, events=events)["root"] ==
            manifest["proposed_root"], "package proposed root differs")
    tests = run(agent, workspace, "test", submitted, events=events)["tests"]
    require(len(tests) == manifest["selected_tests"] and tests and all(t["pass"] for t in tests),
            "package tests differ or fail")
    require(repository_files(workspace) == before, "review changed accepted repository files")
    return manifest, submitted


def build(agent: Path, output: Path, frame_path: Path | None = None) -> dict:
    output.mkdir(parents=True, exist_ok=False)
    events: list = []
    try:
        package = output / "package"
        package.mkdir()
        frame = json.loads(frame_path.read_text()) if frame_path else application_frame()
        write_json(package / "frame.json", frame)
        author = output / "author"
        run(agent, author, "init", "--seed", "24" * 32, events=events)
        base = run(agent, author, "view", events=events)["root"]
        run(agent, author, "export", str(package / "base.pack"), events=events)
        before = repository_files(author)
        trial = run(agent, author, "try", str(package / "frame.json"), "--base-root", base,
                    "--raw", events=events)
        require(trial["verdict"]["valid"] and trial["tests"] and
                trial["verdict"]["selected_tests"] == len(trial["tests"]) and all(t["pass"] for t in trial["tests"]),
                "application candidate/tests must pass")
        view = run(agent, author, "view", "--after", trial["handle"], "--ids", "--types", events=events)
        run(agent, author, "submit", trial["handle"], events=events)
        shutil.copyfile(author / "final_candidate.hex", package / "candidate.hex")
        shutil.copyfile(author / ".sley/names.json", package / "names.json")
        shutil.copyfile(Path(__file__), package / "stock_reorder.py")
        (package / "view.txt").write_text(view["view"], encoding="utf-8")
        refusal = run(agent, author, "commit", trial["handle"], expected=2, events=events)
        require("TXN_TEST_EVIDENCE_UNSUPPORTED" in json.dumps(refusal), "unexpected admission result")
        require(repository_files(author) == before, "build/refusal changed accepted files")
        manifest = {"format": "sley-stock-reorder-v1", "agent_sha256": AGENT_SHA256,
                    "base_root": base, "proposed_root": view["root"],
                    "candidate_stored_sha256": sha(bytes.fromhex(trial["stored_hex"])),
                    "selected_tests": len(trial["tests"]), "tested_candidate_committed": False,
                    "execution": "advisory candidate; host CSV/files supplied by Python",
                    "files": {name: sha((package / name).read_bytes()) for name in sorted(PACKAGE_FILES)}}
        write_json(package / "manifest.json", manifest)
        replay(agent, package, output / "reviewer", events)
        with zipfile.ZipFile(output / "stock-reorder.zip", "w", zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(package.iterdir()):
                archive.write(path, arcname=f"stock-reorder/{path.name}")
        result = {"result": "PASS", "manifest": manifest, "archive_sha256": sha((output / "stock-reorder.zip").read_bytes()),
                  "accepted_files_preserved": True, "admission_refusal": "TXN_TEST_EVIDENCE_UNSUPPORTED"}
        write_json(output / "result.json", result)
        return result
    except (ValueError, TypeError, OSError, KeyError, subprocess.TimeoutExpired) as error:
        write_json(output / "result.json", {"result": "FAIL", "detail": str(error)})
        raise
    finally:
        write_json(output / "transcript.json", events)


def read_inventory(path: Path) -> tuple:
    with path.open("rb") as handle:
        data = handle.read(2_000_001)
    require(len(data) <= 2_000_000, "CSV exceeds 2 MB")
    reader = csv.DictReader(io.StringIO(data.decode("utf-8"), newline=""), strict=True)
    require(reader.fieldnames == ["sku", *FIELDS], "CSV header must be: sku," + ",".join(FIELDS))
    rows, seen = [], set()
    for line, row in enumerate(reader, start=2):
        require(len(rows) < 10_000, "CSV exceeds 10000 rows")
        require(None not in row and all(v is not None for v in row.values()), f"row {line}: wrong field count")
        sku = row["sku"]
        require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,63}", sku) is not None,
                f"row {line}: invalid SKU")
        require(sku not in seen, f"row {line}: duplicate SKU {sku}")
        seen.add(sku)
        args = []
        for name in FIELDS:
            text = row[name]
            require(re.fullmatch(r"-?(0|[1-9][0-9]{0,18})", text) is not None,
                    f"row {line}: {name} must be a signed integer")
            value = int(text)
            require(-2**63 <= value <= I64_MAX, f"row {line}: {name} outside i64")
            args.append(value)
        rows.append((sku, args))
    require(bool(rows), "CSV contains no inventory rows")
    return rows, sha(data)


def calculate(agent: Path, package: Path, rows: list, work: Path) -> tuple:
    manifest, handle = replay(agent, package, work / "workspace")
    inputs = work / "inputs.jsonl"
    inputs.write_text("".join(json.dumps(args) + "\n" for _, args in rows), encoding="utf-8")
    before = repository_files(work / "workspace")
    values = run(agent, work / "workspace", "call", "reorder", "--on", handle,
                 "--batch", str(inputs), lines=True)
    require(len(values) == len(rows), "batch result count differs")
    require(repository_files(work / "workspace") == before, "execution changed accepted files")
    return manifest, values


def report(agent: Path, package: Path, inventory: Path, receipt: Path | None) -> int:
    if receipt:
        require(not receipt.exists(), "receipt already exists")
    rows, inventory_digest = read_inventory(inventory)
    with tempfile.TemporaryDirectory(prefix="sley-stock-reorder-") as directory:
        manifest, values = calculate(agent, package, rows, Path(directory))
    output = io.StringIO(newline="")
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(["sku", "reorder_units", "status"])
    failed = False
    for (sku, _), value in zip(rows, values):
        if set(value) == {"Ok"} and type(value["Ok"]) is int and 0 <= value["Ok"] <= I64_MAX:
            writer.writerow([sku, value["Ok"], "ok"])
        else:
            require(set(value) == {"Err"} and value["Err"] in {"InvalidInput", "Overflow"},
                    "unexpected execution result")
            failed = True
            writer.writerow([sku, "", value["Err"]])
    text = output.getvalue()
    if receipt:
        with receipt.open("x", encoding="utf-8") as handle:
            json.dump({"agent_sha256": AGENT_SHA256, "candidate_stored_sha256": manifest["candidate_stored_sha256"],
                       "host_runner_sha256": sha(Path(__file__).read_bytes()),
                       "base_root": manifest["base_root"], "proposed_root": manifest["proposed_root"],
                       "inventory_sha256": inventory_digest, "csv_sha256": sha(text.encode()),
                       "rows": len(rows), "advisory": True, "results": values}, handle, indent=2)
            handle.write("\n")
    sys.stdout.write(text)
    return 1 if failed else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", required=True, type=Path)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("build", help="create a new package and test it in a fresh workspace")
    create.add_argument("--output", required=True, type=Path)
    create.add_argument("--frame", type=Path, help="reviewed replacement frame; default is the documented rule")
    execute = commands.add_parser("report", help="replay/test a package and emit CSV recommendations")
    execute.add_argument("--package", required=True, type=Path)
    execute.add_argument("--inventory", required=True, type=Path)
    execute.add_argument("--receipt", type=Path)
    args = parser.parse_args()
    try:
        agent = checked_agent(args.agent)
        if args.command == "build":
            print(json.dumps(build(agent, args.output.absolute(), args.frame), indent=2))
            return 0
        return report(agent, args.package.resolve(strict=True), args.inventory, args.receipt)
    except (ValueError, TypeError, OSError, KeyError, csv.Error, subprocess.TimeoutExpired) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

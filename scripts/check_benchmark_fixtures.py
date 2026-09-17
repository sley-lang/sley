#!/usr/bin/env python3
"""S3 gate: every corpus task x every required arm has an honest owner oracle.

For each of the 15 frozen corpus tasks (bench/corpus/v1/tasks.json, never
modified) and each arm (sley2, legacy, raw), this checker verifies the
bench/fixtures/<arm>/<TASK>/ cell: layout, expect.json schema, exclusion
records where authorized, and -- the substance -- it RUNS the oracle:
positive fixture must be accepted (exit 0), every negative must be rejected
with the expected code in the verdict detail (exit 1), exclusions must
reproduce their pinned refusal (exit 1).

Usage: python3 scripts/check_benchmark_fixtures.py [--arm sley2|legacy|raw]
       [--task S2B-...] [--jobs N] [--refresh-manifest] [--oracle-timeout S]
Exit 0: all selected cells green. Exit 1: any failure (details on stdout).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "bench" / "fixtures"
CORPUS = ROOT / "bench" / "corpus" / "v1" / "tasks.json"
MANIFEST = FIXTURES / "manifest.json"
ARMS = ("sley2", "legacy", "raw")
ORACLE_CONTRACT = "sley2.benchmark-task-oracle.v1"
EXCLUSION_TASKS = {"sley2": {"S2B-EFFECT-001", "S2B-CAP-001"}}
IGNORED_NAMES = {"__pycache__"}


def _fail(problems, cell, message):
    problems.append(f"{cell}: {message}")


def _norm(text):
    return re.sub(r"[^a-z0-9]", "", str(text).lower())


def _canonical(value):
    return json.dumps(value, ensure_ascii=False, allow_nan=False,
                      sort_keys=True, separators=(",", ":")).encode("utf-8")


def _fixture_digest(cell_dir):
    digest = hashlib.sha256()
    paths = sorted(p for p in cell_dir.rglob("*")
                   if p.is_file() and not any(
                       part in IGNORED_NAMES or part.endswith(".pyc")
                       for part in p.relative_to(cell_dir).parts))
    for path in paths:
        rel = path.relative_to(cell_dir).as_posix()
        digest.update(rel.encode("utf-8") + b"\0")
        digest.update(path.read_bytes() + b"\0")
    return digest.hexdigest(), len(paths)


def _run_oracle(oracle, fixture_dir, timeout):
    try:
        proc = subprocess.run(
            [sys.executable, str(oracle), str(fixture_dir)],
            cwd=str(ROOT), capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None, "timeout", f"oracle exceeded {timeout}s"
    except OSError as exc:
        return None, "spawn", str(exc)[:200]
    try:
        verdict = json.loads(proc.stdout.strip())
    except (ValueError, AttributeError):
        return proc.returncode, "nonjson", proc.stdout.strip()[:200]
    if not isinstance(verdict, dict):
        return proc.returncode, "nonjson", "verdict is not an object"
    return proc.returncode, verdict, proc.stderr.strip()[:200]


def _check_verdict_shape(verdict, task_id, arm):
    if set(verdict) != {"status", "code", "detail", "task_id", "arm"}:
        return f"verdict field set {sorted(verdict)}"
    if verdict["task_id"] != task_id or verdict["arm"] != arm:
        return "verdict task/arm mismatch"
    if verdict["status"] not in ("accepted", "rejected"):
        return f"bad status {verdict['status']!r}"
    if not isinstance(verdict["detail"], str) or not verdict["detail"]:
        return "empty detail"
    return ""


def _check_cell(task_id, arm, oracle_timeout):
    problems = []
    cell = f"{arm}/{task_id}"
    cell_dir = FIXTURES / arm / task_id
    if not cell_dir.is_dir():
        return [f"{cell}: missing directory"]
    oracle = cell_dir / "oracle.py"
    if not oracle.is_file():
        return [f"{cell}: missing oracle.py"]
    try:
        expect = json.loads((cell_dir / "expect.json").read_text())
    except (OSError, ValueError) as exc:
        return [f"{cell}: expect.json unreadable ({exc})"]
    if expect.get("contract") != ORACLE_CONTRACT:
        _fail(problems, cell, "expect.json contract drift")
    if expect.get("task_id") != task_id or expect.get("arm") != arm:
        _fail(problems, cell, "expect.json task/arm drift")
    excluded = task_id in EXCLUSION_TASKS.get(arm, set())
    if excluded:
        if expect.get("positive") != "rejected" or expect.get("exclusion") is not True:
            _fail(problems, cell, "exclusion must set positive=rejected + exclusion=true")
        try:
            excl = json.loads((cell_dir / "excluded.json").read_text())
        except (OSError, ValueError) as exc:
            _fail(problems, cell, f"excluded.json unreadable ({exc})")
            excl = {}
        for field in ("reason", "refusal_code", "denominator"):
            if not excl.get(field):
                _fail(problems, cell, f"excluded.json missing {field}")
        if excl.get("denominator") != "excluded":
            _fail(problems, cell, "excluded denominator must read 'excluded'")
        negatives = expect.get("negatives", {})
        if negatives:
            _fail(problems, cell, "exclusions carry no negatives")
        for name in negatives:
            if not (cell_dir / "negative" / name).is_dir():
                _fail(problems, cell, f"negative dir missing: {name}")
        rc, verdict, extra = _run_oracle(oracle, cell_dir / "fixture"
                                         if (cell_dir / "fixture").is_dir()
                                         else cell_dir, oracle_timeout)
        if rc != 1 or not isinstance(verdict, dict) or verdict.get("status") != "rejected":
            _fail(problems, cell, f"exclusion oracle must exit 1/rejected (rc={rc} {verdict})")
        elif verdict.get("code") != excl.get("refusal_code"):
            _fail(problems, cell, f"exclusion refusal drift: {verdict.get('code')}")
        elif _check_verdict_shape(verdict, task_id, arm):
            _fail(problems, cell, _check_verdict_shape(verdict, task_id, arm))
        return problems
    if expect.get("positive") != "accepted":
        _fail(problems, cell, "non-excluded positive must read 'accepted'")
    negatives = expect.get("negatives", {})
    if not isinstance(negatives, dict) or not negatives:
        _fail(problems, cell, "at least one negative control is required")
    for name in negatives:
        if not (cell_dir / "negative" / name).is_dir():
            _fail(problems, cell, f"negative dir missing: {name}")
    if not (cell_dir / "fixture").is_dir():
        _fail(problems, cell, "missing fixture/ directory")
    lim_path = cell_dir / "arm_limitation.json"
    if lim_path.exists():
        try:
            lims = json.loads(lim_path.read_text())
        except ValueError:
            _fail(problems, cell, "arm_limitation.json is not JSON")
            lims = []
        if not isinstance(lims, list) or not lims or not all(
                isinstance(entry, dict) and entry.get("capability")
                and entry.get("reason") and entry.get("oracle_asserts")
                for entry in lims):
            _fail(problems, cell, "arm_limitation.json needs capability/reason/oracle_asserts entries")
    if problems:
        return problems
    rc, verdict, extra = _run_oracle(oracle, cell_dir / "fixture", oracle_timeout)
    if rc != 0 or not isinstance(verdict, dict) or verdict.get("status") != "accepted":
        _fail(problems, cell, f"positive must exit 0/accepted (rc={rc} {verdict} {extra})")
        return problems
    shape = _check_verdict_shape(verdict, task_id, arm)
    if shape:
        _fail(problems, cell, f"positive {shape}")
    for name, expected_code in negatives.items():
        rc, verdict, extra = _run_oracle(
            oracle, cell_dir / "negative" / name, oracle_timeout)
        if rc != 1 or not isinstance(verdict, dict) or verdict.get("status") != "rejected":
            _fail(problems, cell, f"negative {name} must exit 1/rejected (rc={rc} {verdict} {extra})")
            continue
        shape = _check_verdict_shape(verdict, task_id, arm)
        if shape:
            _fail(problems, cell, f"negative {name} {shape}")
            continue
        if verdict.get("code") != expected_code:
            _fail(problems, cell, f"negative {name} code {verdict.get('code')!r} != {expected_code!r}")
            continue
        if _norm(expected_code) not in _norm(verdict.get("detail", "")):
            _fail(problems, cell, f"negative {name} detail does not evidence {expected_code!r}")
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arm", choices=ARMS)
    parser.add_argument("--task")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--refresh-manifest", action="store_true")
    parser.add_argument("--oracle-timeout", type=float, default=900.0)
    args = parser.parse_args()
    try:
        corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
        task_ids = [t["id"] for t in corpus["tasks"]]
    except (OSError, ValueError, KeyError) as exc:
        print(json.dumps({"result": "FAIL", "problems": [f"corpus unreadable: {exc}"]}))
        return 1
    if len(task_ids) != 15:
        print(json.dumps({"result": "FAIL", "problems": [f"corpus task count {len(task_ids)} != 15 (frozen corpus drift?)"]}))
        return 1
    arms = [args.arm] if args.arm else list(ARMS)
    if args.task:
        task_ids = [t for t in task_ids if t == args.task]
        if not task_ids:
            print(json.dumps({"result": "FAIL", "problems": ["unknown task"]}))
            return 1
    cells = [(t, a) for t in task_ids for a in arms]
    problems: list[str] = []
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        futures = {pool.submit(_check_cell, t, a, args.oracle_timeout): (t, a) for t, a in cells}
        results = {}
        for future in futures:
            t, a = futures[future]
            try:
                results[(t, a)] = future.result()
            except Exception as exc:  # never let a cell crash the gate
                results[(t, a)] = [f"{a}/{t}: checker crash {type(exc).__name__}: {exc}"]
    for t, a in cells:
        problems.extend(results[(t, a)])
    manifest_ok = True
    digests = {}
    for t, a in cells:
        cell_dir = FIXTURES / a / t
        if cell_dir.is_dir():
            digest, count = _fixture_digest(cell_dir)
            digests[f"{a}/{t}"] = {"sha256": digest, "files": count}
    if args.refresh_manifest:
        MANIFEST.write_text(_canonical(
            {"contract": "sley2.benchmark-fixture-manifest.v1",
             "corpus_sha256": hashlib.sha256(CORPUS.read_bytes()).hexdigest(),
             "cells": digests}).decode("utf-8") + "\n", encoding="utf-8")
    elif MANIFEST.is_file():
        try:
            recorded = json.loads(MANIFEST.read_text(encoding="utf-8"))
        except ValueError:
            problems.append("manifest.json is not JSON")
            recorded = {}
        if recorded.get("corpus_sha256") != hashlib.sha256(CORPUS.read_bytes()).hexdigest():
            problems.append("manifest corpus_sha256 drift (frozen corpus changed?)")
        for key, entry in digests.items():
            if recorded.get("cells", {}).get(key, {}).get("sha256") != entry["sha256"]:
                problems.append(f"manifest digest drift: {key}")
                manifest_ok = False
    else:
        problems.append("manifest.json missing (run --refresh-manifest once, then commit it)")
    report = {"result": "PASS" if not problems else "FAIL",
              "cells": len(cells),
              "cells_passing": sum(1 for t, a in cells if not results[(t, a)]),
              "manifest": "ok" if manifest_ok and MANIFEST.is_file() else "drift",
              "problems": sorted(problems)}
    print(json.dumps(report, indent=2))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

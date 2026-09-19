#!/usr/bin/env python3
"""Validate a persistent-fuzz slice's durable proof record against HEAD.

Every `s20_700_*` slice (and the S20-350 mutation-candidate slice) records
`last_local_proof` in the machine summary. Until db53894e only the VM
checker validated it (Vulcan V-01 at 178873d7 / db53894e); the other
checkers pinned markers and accepted any proof, so a proof bound to a
commit outside HEAD's history passed. This module is the one validation
every slice checker calls:

- the record is a PASS with a 40-hex `source_commit` that is an ancestor of
  HEAD;
- `executed_runs` meets `runs_floor` (per target for multi-target slices),
  `new_crash_artifacts` is empty, and the owner library was instrumented
  (`owner_lib_sancov` > 0) when the record carries that figure;
- the lane inputs did not change since the proof commit: the fuzz target
  sources, the runner, `fuzz/Cargo.toml`, `fuzz/Cargo.lock`, and every
  workspace crate the targets reach transitively through `Cargo.lock`
  (Vulcan P3 at db53894e: the VM target links sley-check, sley-ssmc,
  sley-id, sley-mutate and sley-scb1 besides sley-vm).
"""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

WORKSPACE_PREFIX = "sley-"


def lock_packages(root: Path) -> dict[str, list[str]]:
    """Cargo.lock as {package: [dependency names]} (leading name token only)."""
    packages: dict[str, list[str]] = {}
    name = None
    deps: list[str] = []
    in_deps = False
    for line in (root / "Cargo.lock").read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped == "[[package]]":
            if name is not None:
                packages[name] = deps
            name, deps, in_deps = None, [], False
        elif stripped.startswith("name = "):
            name = stripped[len("name = "):].strip().strip('"')
        elif stripped == "dependencies = [":
            in_deps = True
        elif in_deps and stripped == "]":
            in_deps = False
        elif in_deps and name is not None:
            token = stripped.strip(",").strip('"').split()
            if token:
                deps.append(token[0])
    if name is not None:
        packages[name] = deps
    return packages


def target_crates(root: Path, binaries: list[str]) -> list[str]:
    """Workspace crates the fuzz targets reach, transitively, via Cargo.lock."""
    packages = lock_packages(root)
    roots: set[str] = set()
    for binary in binaries:
        source = (root / "fuzz/targets" / f"{binary}.rs").read_text(encoding="utf-8")
        for crate in re.findall(r"\b(sley_[a-z0-9_]+)\b", source):
            roots.add(crate.replace("_", "-"))
    reached: set[str] = set()
    queue = sorted(roots)
    while queue:
        crate = queue.pop()
        if crate in reached or not crate.startswith(WORKSPACE_PREFIX):
            continue
        reached.add(crate)
        queue.extend(dep for dep in packages.get(crate, []) if dep.startswith(WORKSPACE_PREFIX))
    return sorted(reached)


def lane_paths(root: Path, runner: str, binaries: list[str]) -> list[str]:
    paths = [f"fuzz/targets/{binary}.rs" for binary in binaries]
    paths += [runner, "fuzz/Cargo.toml", "fuzz/Cargo.lock"]
    paths += [f"crates/{crate}" for crate in target_crates(root, binaries) if (root / "crates" / crate).is_dir()]
    return paths


def _all_empty(value: object) -> bool:
    if isinstance(value, dict):
        return all(_all_empty(item) for item in value.values())
    return not value


def proof_record_problems(root: Path, proof: object, runner: str, binaries: list[str]) -> list[str]:
    """Problems of one slice's `last_local_proof` (empty when it is a valid, fresh PASS)."""
    problems: list[str] = []
    if not isinstance(proof, dict):
        return ["proof-record-missing"]
    if proof.get("result") != "PASS":
        problems.append("proof-record-not-pass")
    runs = proof.get("executed_runs")
    floor = proof.get("runs_floor")
    if isinstance(runs, dict):
        targets = proof.get("targets") if isinstance(proof.get("targets"), dict) else {}
        for name, count in runs.items():
            target_floor = (targets.get(name) or {}).get("runs_floor", floor)
            if not isinstance(count, int):
                problems.append(f"proof-record-not-int:executed_runs:{name}")
            elif isinstance(target_floor, int) and count < target_floor:
                problems.append(f"proof-record-below-floor:{name}")
            elif not isinstance(target_floor, int):
                problems.append(f"proof-record-not-int:runs_floor:{name}")
    else:
        for key, value in (("executed_runs", runs), ("runs_floor", floor)):
            if not isinstance(value, int):
                problems.append(f"proof-record-not-int:{key}")
        if isinstance(runs, int) and isinstance(floor, int) and runs < floor:
            problems.append("proof-record-below-floor")
    # Every runner emits both keys; a record without them is not a proof
    # (Vulcan P3 at 76227765: the shared validator had accepted absent keys).
    if "new_crash_artifacts" not in proof:
        problems.append("proof-record-missing:new_crash_artifacts")
    elif not _all_empty(proof.get("new_crash_artifacts")):
        problems.append("proof-record-new-crashes")
    if not isinstance(proof.get("owner_lib_sancov"), int) or proof["owner_lib_sancov"] <= 0:
        problems.append("proof-record-no-owner-sancov")
    commit = proof.get("source_commit")
    if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit):
        problems.append("proof-record-bad-source-commit")
        return problems
    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", commit, "HEAD"], cwd=root, capture_output=True, text=True, check=False
    )
    if ancestor.returncode != 0:
        problems.append("proof-record-not-ancestor")
        return problems
    fresh = subprocess.run(
        ["git", "diff", "--quiet", commit, "HEAD", "--", *lane_paths(root, runner, binaries)],
        cwd=root, capture_output=True, text=True, check=False,
    )
    if fresh.returncode != 0:
        problems.append("proof-record-predates-lane-change")
    return problems


def slice_proof_problems(root: Path, section: str, runner: str, binaries: list[str]) -> list[str]:
    summary = json.loads((root / "machineresearch/sley-2.0/machine-summary.json").read_text(encoding="utf-8"))
    proof = (summary.get(section) or {}).get("last_local_proof")
    return proof_record_problems(root, proof, runner, binaries)

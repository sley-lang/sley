#!/usr/bin/env python3
"""Check the native test owners' one-way workspace dependency graph."""

from __future__ import annotations

import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def graph() -> dict[str, set[str]]:
    manifests = sorted((ROOT / "crates").glob("*/Cargo.toml"))
    packages = {
        tomllib.loads(path.read_text())["package"]["name"]: path for path in manifests
    }
    edges: dict[str, set[str]] = {}
    for name, path in packages.items():
        dependencies = tomllib.loads(path.read_text()).get("dependencies", {})
        targets = {
            value.get("package", dependency) if isinstance(value, dict) else dependency
            for dependency, value in dependencies.items()
        }
        edges[name] = targets & packages.keys()
    return edges


def reachable(edges: dict[str, set[str]], start: str) -> set[str]:
    visited: set[str] = set()
    pending = list(edges[start])
    while pending:
        name = pending.pop()
        if name in visited:
            continue
        visited.add(name)
        pending.extend(edges[name] - visited)
    return visited


def main() -> None:
    edges = graph()
    forbidden = {
        "sley-vm": {"sley-tests", "sley-policy", "sley-conformance", "sley-test-runner"},
        "sley-tests": {
            "sley-policy", "sley-txn", "sley-repo", "sley-protocol", "sley-test-runner"
        },
        "sley-test-runner": {"sley-policy", "sley-txn", "sley-repo", "sley-protocol"},
    }
    problems = []
    for owner, excluded in forbidden.items():
        if owner not in edges:
            problems.append(f"missing owner: {owner}")
            continue
        leaked = reachable(edges, owner) & excluded
        problems.extend(f"forbidden dependency: {owner} -> {name}" for name in sorted(leaked))
    for name in edges:
        if name in reachable(edges, name):
            problems.append(f"dependency cycle through {name}")
    if problems:
        raise SystemExit("\n".join(problems))
    print("native test dependency graph: ok")


if __name__ == "__main__":
    main()

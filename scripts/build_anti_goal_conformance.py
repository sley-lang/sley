#!/usr/bin/env python3
"""Measure the constitutional anti-goal matrix mechanically.

`docs/ANTI_GOALS.md` states that "each prohibition must remain mechanically
testable or independently reviewable" and names the acceptance evidence for
twenty-six anti-goals, but nothing evaluated them. This report evaluates every
anti-goal whose evidence is mechanical and marks the rest as review-only, so
master goal section 28 has a tracked state instead of prose.

It judges nothing that needs a human: a `REVIEW_ONLY` verdict is not a pass.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MATRIX = ROOT / "docs/ANTI_GOALS.md"
REPORT = ROOT / "evidence/validation/anti-goal-conformance.json"
CONTRACT = "sley2.anti-goal-conformance.v1"
KERNEL_TREES = ("crates",)
# Crate names that would signal a prohibited capability if they entered the
# dependency graph.
FORBIDDEN_DEPENDENCIES = {
    "parser": ("pest", "nom", "lalrpop", "chumsky", "combine", "tree-sitter", "logos", "peg"),
    "language_service": ("tower-lsp", "lsp-types", "lsp-server", "rustyline", "reedline"),
    "native_codegen": ("cranelift", "inkwell", "llvm-sys", "wasmtime", "wasmer", "cranelift-jit"),
    "network": ("reqwest", "hyper", "tokio", "async-std", "ureq", "curl", "rustls", "socket2"),
    "greyforge_product": ("zjx", "siglum", "forge", "openclaw"),
}


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def matrix_rows() -> list[dict[str, str]]:
    rows = []
    for line in MATRIX.read_text(encoding="utf-8").split("\n"):
        if not line.startswith("| ") or line.startswith("| Anti-goal") or set(line) <= set("|- "):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 3:
            continue
        rows.append({"anti_goal": cells[0], "surface": cells[1], "acceptance_evidence": cells[2]})
    return rows


def locked_dependencies() -> set[str]:
    return set(re.findall(r'^name = "([a-z0-9_-]+)"', (ROOT / "Cargo.lock").read_text(encoding="utf-8"), re.M))


def kernel_sources() -> list[Path]:
    sources = []
    for tree in KERNEL_TREES:
        for path in sorted((ROOT / tree).rglob("*.rs")):
            if "/target/" not in str(path):
                sources.append(path)
    return sources


def git(*arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments], cwd=ROOT, capture_output=True, text=True, check=False
    ).stdout.strip()


def evaluate() -> dict[str, dict]:
    """One verdict per mechanically checkable anti-goal."""
    locked = locked_dependencies()
    sources = kernel_sources()
    verdicts: dict[str, dict] = {}

    def record(anti_goal: str, holds: bool, detail: str) -> None:
        verdicts[anti_goal] = {"verdict": "HOLDS" if holds else "VIOLATED", "detail": detail}

    sley_files = [
        str(path.relative_to(ROOT))
        for path in ROOT.rglob("*.sley")
        if "/target/" not in str(path)
    ]
    parsers = sorted(locked & set(FORBIDDEN_DEPENDENCIES["parser"]))
    record(
        "Sley source syntax or parser",
        not sley_files and not parsers,
        f"{len(sley_files)} .sley files; parser crates in the lock: {parsers or 'none'}",
    )

    services = sorted(locked & set(FORBIDDEN_DEPENDENCIES["language_service"]))
    cli = (ROOT / "crates/sley-cli/src/lib.rs").read_text(encoding="utf-8")
    surfaces = [word for word in ("\"fmt\"", "\"repl\"", "\"lsp\"", "\"format\"") if word in cli]
    record(
        "formatter, REPL, Tree-sitter, conventional LSP",
        not services and not surfaces,
        f"language-service crates: {services or 'none'}; CLI command literals: {surfaces or 'none'}",
    )

    native = sorted(locked & set(FORBIDDEN_DEPENDENCIES["native_codegen"]))
    record(
        "native/JIT/AOT/marketplace/self-hosting before GA",
        not native,
        f"codegen or runtime crates in the lock: {native or 'none'}",
    )

    network = sorted(locked & set(FORBIDDEN_DEPENDENCIES["network"]))
    record(
        "network-dependent core tests",
        not network,
        f"networking crates in the lock: {network or 'none'}; the lock holds {len(locked)} crates in total",
    )

    products = sorted(
        dependency
        for dependency in locked
        if any(product in dependency for product in FORBIDDEN_DEPENDENCIES["greyforge_product"])
    )
    record(
        "mandatory Greyforge dependency",
        not products,
        f"Greyforge product crates in the lock: {products or 'none'}",
    )

    shell_users = [
        str(path.relative_to(ROOT))
        for path in sources
        if "std::process::Command" in path.read_text(encoding="utf-8", errors="ignore")
    ]
    record(
        "arbitrary shell",
        not shell_users,
        f"kernel sources invoking a process: {shell_users or 'none'}",
    )

    workspace_manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    unsafe_sources = [
        str(path.relative_to(ROOT))
        for path in sources
        if re.search(r"^\s*(unsafe |#!\[allow\(unsafe_code)", path.read_text(encoding="utf-8", errors="ignore"), re.M)
    ]
    record(
        "unsafe code hidden in kernel",
        'unsafe_code = "forbid"' in workspace_manifest and not unsafe_sources,
        f"workspace lint forbids unsafe: {'unsafe_code = \"forbid\"' in workspace_manifest}; "
        f"kernel sources with unsafe or an allow: {unsafe_sources or 'none'}",
    )

    tags = [tag for tag in git("tag").split("\n") if tag]
    unpushed = git("log", "origin/main..HEAD", "--oneline").count("\n")
    record(
        "unauthorized publication/deploy/spend",
        not tags,
        f"git tags: {len(tags)}; commits not pushed to origin/main: {unpushed + 1 if unpushed else 0}",
    )

    legacy = sorted(
        dependency for dependency in locked if dependency.startswith("sley1") or dependency == "sley"
    )
    record(
        "Sley 1.x compatibility",
        not legacy,
        f"legacy crates in the lock: {legacy or 'none'}",
    )

    return verdicts


def build_report() -> dict:
    rows = matrix_rows()
    verdicts = evaluate()
    entries = []
    for row in rows:
        verdict = verdicts.get(row["anti_goal"])
        entries.append(
            {
                **row,
                "state": verdict["verdict"] if verdict else "REVIEW_ONLY",
                "detail": verdict["detail"] if verdict else "no mechanical evaluation; the Council review owns it",
            }
        )
    states: dict[str, int] = {}
    for entry in entries:
        states[entry["state"]] = states.get(entry["state"], 0) + 1
    report = {
        "contract": CONTRACT,
        "source": "docs/ANTI_GOALS.md",
        "anti_goal_count": len(entries),
        "states": states,
        "violations": [entry["anti_goal"] for entry in entries if entry["state"] == "VIOLATED"],
        "anti_goals": entries,
        "interpretation": (
            "HOLDS means the named mechanical evidence was evaluated and passed; REVIEW_ONLY means "
            "the anti-goal's evidence needs a human judgment and is not a pass. A VIOLATED entry is "
            "a constitutional breach and fails the gate."
        ),
    }
    report["report_digest"] = digest_of(report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    report = build_report()
    text = canonical(report)
    summary = {
        "anti_goal_count": report["anti_goal_count"],
        "states": report["states"],
        "violations": report["violations"],
    }
    if report["violations"]:
        print(canonical({"result": "FAIL", **summary}), end="")
        return 1
    if arguments.check:
        current = REPORT.read_text(encoding="utf-8") if REPORT.exists() else None
        if current != text:
            print(
                canonical(
                    {
                        "mode": "check",
                        "result": "FAIL",
                        "detail": "the tracked anti-goal report differs from the derived report",
                    }
                ),
                end="",
            )
            return 1
        print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
        return 0
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(text, encoding="utf-8")
    print(canonical({"mode": "write", "result": "PASS", **summary}), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())

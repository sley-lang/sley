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
PROBE_SOURCE = Path("crates/sley-test-runner/src/probe.rs")
# Crate names that would signal a prohibited capability if they entered the
# dependency graph.
FORBIDDEN_DEPENDENCIES = {
    "parser": ("pest", "nom", "lalrpop", "chumsky", "combine", "tree-sitter", "logos", "peg"),
    "language_service": ("tower-lsp", "lsp-types", "lsp-server", "rustyline", "reedline"),
    "native_codegen": ("cranelift", "inkwell", "llvm-sys", "wasmtime", "wasmer", "cranelift-jit"),
    "network": ("reqwest", "hyper", "tokio", "async-std", "ureq", "curl", "rustls", "socket2"),
    "greyforge_product": ("zjx", "siglum", "forge", "openclaw"),
}
# The authorized campaign boundary record that declared SH2 work items must
# cite (REWEAVE-1.0 scope adoption, ADR-0049). The record itself is chartered
# by RW-030; until it exists, no SH2 work item can be authorized.
BOUNDARY_RECORD = "host-boundary.json"
SH2_WORK_ITEMS = Path("evidence") / "reweave" / "sh2-work-items.json"
SH2_WORK_ITEMS_CONTRACT = "sley2.reweave-sh2-work-items.v1"


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


def approved_probe_process_surface(source: str) -> bool:
    """Whether the host-readiness probe keeps its closed shell-free surface."""
    if source.count("std::process::Command::new(") != 1:
        return False
    if "fn run_capture(program: &str, args: &[&str])" not in source:
        return False
    if "std::process::Command::new(program)\n        .args(args)\n        .output()" not in source:
        return False
    calls = re.findall(r'run_capture\("([^"]+)",\s*&\[([^]]*)\]\)', source)
    if sorted(calls) != [
        ("getconf", '"PAGESIZE"'),
        ("systemctl", '"--version"'),
    ]:
        return False
    return not re.search(r'Command::new\("(?:ba|da|z)?sh"\)|"-c"', source)


def git(*arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments], cwd=ROOT, capture_output=True, text=True, check=False
    ).stdout.strip()


def evaluate_campaign_declarations(root: Path) -> tuple[bool, str]:
    """Review-gated campaign-declaration check for SH2 work items.

    Declared SH2 work items live in the registry at
    `evidence/reweave/sh2-work-items.json`; every item must cite the
    authorized campaign boundary record (`host-boundary.json`) by exact
    path and SHA-256 digest and must name its staged SH2 gate. This check
    verifies citation presence and correctness only; semantic authorization
    of the work stays with human review. An absent registry means no SH2
    work is declared, which holds vacuously. The six-crate native-codegen
    denylist evaluated alongside this check still fails unconditionally.
    """
    registry = root / SH2_WORK_ITEMS
    if not registry.exists():
        return True, "no declared SH2 work items"
    try:
        declarations = json.loads(registry.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return False, f"declared SH2 work items unreadable: {error}"
    if not isinstance(declarations, dict) or declarations.get("contract") != SH2_WORK_ITEMS_CONTRACT:
        return False, "declared SH2 work items carry the wrong contract"
    items = declarations.get("items")
    if not isinstance(items, list):
        return False, "declared SH2 work items carry no item list"
    boundary = root / BOUNDARY_RECORD
    if items and not boundary.is_file():
        return False, (
            f"{len(items)} declared SH2 work items but no authorized {BOUNDARY_RECORD}"
        )
    try:
        boundary_bytes = boundary.read_bytes() if items else None
    except OSError as error:
        return False, f"authorized {BOUNDARY_RECORD} unreadable: {error}"
    digest = hashlib.sha256(boundary_bytes).hexdigest() if boundary_bytes is not None else None
    for item in items:
        if not isinstance(item, dict):
            return False, "a declared SH2 work item is not an object"
        cited = item.get("boundary_record")
        cited = cited if isinstance(cited, dict) else {}
        if cited.get("path") != BOUNDARY_RECORD or cited.get("sha256") != digest:
            return False, (
                f"SH2 work item {item.get('id', '?')} does not cite "
                f"the authorized {BOUNDARY_RECORD} digest"
            )
        gate = item.get("gate")
        if not isinstance(gate, str) or not gate.strip():
            return False, f"SH2 work item {item.get('id', '?')} names no staged SH2 gate"
    return True, (
        f"{len(items)} declared SH2 work items cite the authorized "
        "boundary record and a staged gate"
    )


def evaluate() -> dict[str, dict]:
    """One verdict per mechanically checkable anti-goal."""
    locked = locked_dependencies()
    sources = kernel_sources()
    verdicts: dict[str, dict] = {}

    def record(anti_goal: str, holds: bool, detail: str) -> None:
        verdicts[anti_goal] = {"verdict": "HOLDS" if holds else "VIOLATED", "detail": detail}

    fixture_root = ROOT / "bench" / "fixtures"
    sley_files = [
        str(path.relative_to(ROOT))
        for path in ROOT.rglob("*.sley")
        if "/target/" not in str(path) and not path.is_relative_to(fixture_root)
    ]
    parsers = sorted(locked & set(FORBIDDEN_DEPENDENCIES["parser"]))
    record(
        "Sley source syntax or parser",
        not sley_files and not parsers,
        f"{len(sley_files)} production .sley files; parser crates in the lock: {parsers or 'none'}; "
        "benchmark fixture sources are inert corpus inputs",
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
    campaign_ok, campaign_detail = evaluate_campaign_declarations(ROOT)
    record(
        "native/JIT/AOT/marketplace/self-hosting outside an authorized campaign",
        not native and campaign_ok,
        f"codegen or runtime crates in the lock: {native or 'none'}; {campaign_detail}",
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

    shell_users = []
    for path in sources:
        source = path.read_text(encoding="utf-8", errors="ignore")
        if "std::process::Command" not in source:
            continue
        relative = path.relative_to(ROOT)
        if relative == PROBE_SOURCE and approved_probe_process_surface(source):
            continue
        shell_users.append(str(relative))
    record(
        "arbitrary shell",
        not shell_users,
        f"unapproved process surfaces: {shell_users or 'none'}; the host-readiness probe is "
        "limited to systemctl --version and getconf PAGESIZE without a shell",
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
    # The unpushed-commit count moves with every commit, so it made this
    # derived report drift at each checkpoint; scripts/check_remote_consistency.py
    # reports it at handoff instead.
    record(
        "unauthorized publication/deploy/spend",
        not tags,
        f"git tags: {len(tags)}; unpushed commits are reported by scripts/check_remote_consistency.py",
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

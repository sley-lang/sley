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
import functools
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import publication_authority  # noqa: E402  (sibling module)
from rust_source_regions import rust_code_mask  # noqa: E402  (sibling module)


ROOT = Path(__file__).resolve().parents[1]
MATRIX = ROOT / "docs/ANTI_GOALS.md"
REPORT = ROOT / "evidence/validation/anti-goal-conformance.json"
CONTRACT = "sley2.anti-goal-conformance.v1"
KERNEL_TREES = ("crates",)
PROBE_SOURCE = Path("crates/sley-test-runner/src/probe.rs")
# The one unsafe exception (ADR-0052): a module of the sley-agent binary target.
AGENT_CRATE = Path("crates/sley-agent")
ALLOCATOR_SOURCE = AGENT_CRATE / "src/allocator.rs"
ALLOCATOR_ADR = Path("docs/adr/ADR-0052-agent-binary-allocator.md")
# Code (comments and literals masked) that uses or permits unsafe code: the
# keyword itself (blocks, fns, impls, traits, extern blocks, unsafe
# attributes) or a lint level that relaxes `unsafe_code`.
UNSAFE_TOKEN = re.compile(r"\bunsafe\b")
UNSAFE_LINT_RELAX = re.compile(r"\b(?:allow|expect|warn)\s*\([^)]*\bunsafe_code\b")
# What the allocator module may not contain: other files or code pulled in,
# foreign or exported symbols, mutable statics, transmutes, installation.
ALLOCATOR_FORBIDDEN = {
    "an out-of-line module": r"\bmod\s+\w+\s*;",
    "a #[path] attribute": r"#\s*!?\s*\[\s*path\b",
    "include!": r"\binclude(?:_str|_bytes)?\s*!",
    "macro_rules!": r"\bmacro_rules\s*!",
    "an exported or placed symbol": r"\b(?:no_mangle|export_name|link_section|link_name|used)\b",
    "extern": r"\bextern\b",
    "inline assembly": r"\b(?:global_)?asm\s*!",
    "a mutable static": r"\bstatic\s+mut\b",
    "transmute": r"\btransmute\b",
    "from_raw_parts_mut": r"\bfrom_raw_parts_mut\b",
    "#[global_allocator]": r"\bglobal_allocator\b",
}
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
# The optional familiar authoring frontend (ADR-0055): the one parser of
# program text, confined to one feature-gated module of the agent workbench.
FAMILIAR_FEATURE = "familiar"
FAMILIAR_SOURCE = AGENT_CRATE / "src/familiar.rs"
FAMILIAR_ADR = Path("docs/adr/ADR-0055-familiar-authoring-frontend.md")
FAMILIAR_GATE = re.compile(r'#\[cfg\(feature\s*=\s*"familiar"\)\]\s*pub(?:\(crate\))?\s+mod\s+familiar\s*;')
FAMILIAR_IMPORT = re.compile(r"^\s*use\s+([A-Za-z_][A-Za-z0-9_]*)", re.M)
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


def rust_code(source: str) -> str:
    """The source with comments and literals blanked (newlines kept)."""
    return "".join(
        char if keep or char == "\n" else " " for char, keep in zip(source, rust_code_mask(source))
    )


@functools.cache
def code_of(path: Path) -> str:
    """`rust_code` of a file, lexed once per run."""
    return rust_code(path.read_text(encoding="utf-8", errors="ignore"))


CFG_TEST_MODULE = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{")


def without_test_modules(source: str) -> str:
    """The source with every inline `#[cfg(test)] mod name { ... }` blanked.

    Test modules compile only into test binaries, never into a shipped
    kernel or agent executable. Braces are matched on the masked code, so
    braces inside comments and literals do not count; an unbalanced module
    is left in place (it then still counts, which fails closed).
    """
    code = rust_code(source)
    out = list(source)
    for match in CFG_TEST_MODULE.finditer(code):
        depth, end = 0, None
        for index in range(match.end() - 1, len(code)):
            if code[index] == "{":
                depth += 1
            elif code[index] == "}":
                depth -= 1
                if depth == 0:
                    end = index
                    break
        if end is None:
            continue
        for index in range(match.start(), end + 1):
            if out[index] != "\n":
                out[index] = " "
    return "".join(out)


def uses_unsafe(code: str) -> bool:
    return bool(UNSAFE_TOKEN.search(code) or UNSAFE_LINT_RELAX.search(code))


def allocator_exception_problems(sources: list[Path]) -> list[str]:
    """Why the ADR-0052 exception does not hold; empty when it does.

    The exception is exactly one module of the sley-agent binary target,
    installed by the binary and by the workbench tests only; the library
    forbids unsafe code; the package relaxes only `unsafe_code`, to `deny`;
    no other member relaxes anything; and the module pulls in no other code
    and exports nothing."""
    problems = []
    if not (ROOT / ALLOCATOR_ADR).is_file():
        problems.append(f"{ALLOCATOR_ADR} is missing")
    allocator = rust_code((ROOT / ALLOCATOR_SOURCE).read_text(encoding="utf-8"))
    relaxations = UNSAFE_LINT_RELAX.findall(allocator)
    if len(relaxations) != 1 or len(re.findall(r"#!\[allow\(unsafe_code\)\]", allocator)) != 1:
        problems.append("the allocator module must carry exactly one #![allow(unsafe_code)] and no other relaxation")
    if "unsafe impl GlobalAlloc for SizeClassCache" not in " ".join(allocator.split()):
        problems.append("the allocator module must implement GlobalAlloc for SizeClassCache")
    for what, pattern in ALLOCATOR_FORBIDDEN.items():
        if re.search(pattern, allocator):
            problems.append(f"the allocator module contains {what}")
    library = rust_code((ROOT / AGENT_CRATE / "src/lib.rs").read_text(encoding="utf-8"))
    if "#![forbid(unsafe_code)]" not in library or re.search(r"\ballocator\b|#\s*\[\s*path\b", library):
        problems.append("sley-agent's lib.rs must forbid unsafe code and must not include the allocator")
    binary = rust_code((ROOT / AGENT_CRATE / "src/main.rs").read_text(encoding="utf-8"))
    if not re.search(r"^mod allocator;$", binary, re.M) or uses_unsafe(binary):
        problems.append("sley-agent's main.rs must declare `mod allocator;` and use no unsafe code")
    for path in sources:
        relative = path.relative_to(ROOT)
        code = code_of(path)
        installs = len(re.findall(r"\bglobal_allocator\b", code))
        includes = re.findall(r"#\s*\[\s*path\s*=\s*\"([^\"]*)\"", path.read_text(encoding="utf-8", errors="ignore"))
        allowed_site = relative in (AGENT_CRATE / "src/main.rs", AGENT_CRATE / "tests/workbench.rs")
        if installs and not (allowed_site and installs == 1):
            problems.append(f"{relative} installs a global allocator")
        if any(include.endswith("allocator.rs") for include in includes) and relative != AGENT_CRATE / "tests/workbench.rs":
            problems.append(f"{relative} includes the allocator module")
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    expected = {
        "rust": {**workspace["workspace"]["lints"]["rust"], "unsafe_code": "deny"},
        "clippy": workspace["workspace"]["lints"]["clippy"],
    }
    for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        package = tomllib.loads(manifest.read_text(encoding="utf-8"))
        agent = manifest.parent == ROOT / AGENT_CRATE
        if package.get("lints") != (expected if agent else {"workspace": True}):
            problems.append(f"{manifest.relative_to(ROOT)} has lints {package.get('lints')}")
        if "build" in package.get("package", {}) or (manifest.parent / "build.rs").exists():
            if agent:
                problems.append("sley-agent has a build script")
    return problems


def kernel_sources() -> list[Path]:
    sources = []
    for tree in KERNEL_TREES:
        for path in sorted((ROOT / tree).rglob("*.rs")):
            if "target" not in path.relative_to(ROOT).parts:
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


def unsafe_row(sources: list[Path]) -> tuple[bool, str]:
    """The anti-goal "unsafe code hidden in kernel": the workspace forbids
    unsafe code, and no source uses or permits it except the ADR-0052
    allocator module while its exception holds."""
    workspace_manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    forbid = 'unsafe_code = "forbid"' in workspace_manifest
    problems = allocator_exception_problems(sources) if (ROOT / ALLOCATOR_SOURCE).exists() else []
    exempt = {ALLOCATOR_SOURCE} if (ROOT / ALLOCATOR_SOURCE).exists() and not problems else set()
    unsafe_sources = [
        str(path.relative_to(ROOT))
        for path in sources
        if path.relative_to(ROOT) not in exempt
        and uses_unsafe(code_of(path))
    ]
    detail = (
        f"workspace lint forbids unsafe: {forbid}; "
        f"sources that use or permit unsafe code: {unsafe_sources or 'none'}; "
        f"exception isolated by ADR-0052: {[str(path) for path in sorted(exempt)] or 'none'}"
    )
    if problems:
        detail += f"; ADR-0052 exception problems: {problems}"
    return forbid and not unsafe_sources and not problems, detail


def familiar_frontend_problems() -> list[str]:
    """Why the ADR-0055 frontend confinement does not hold; empty when it does.

    The frontend is one module of the sley-agent crate behind its optional
    `familiar` feature, which enables no dependency; the module imports only
    `std` and `serde_json`, so it can produce a JSON frame and nothing else
    (no kernel, candidate, store or commit API); and no other crate depends
    on the agent crate or names the module.
    """
    problems: list[str] = []
    if not (ROOT / FAMILIAR_ADR).is_file():
        problems.append(f"{FAMILIAR_ADR} is missing")
    manifest = tomllib.loads((ROOT / AGENT_CRATE / "Cargo.toml").read_text(encoding="utf-8"))
    features = manifest.get("features", {})
    if (ROOT / FAMILIAR_SOURCE).exists():
        if features.get(FAMILIAR_FEATURE) != []:
            problems.append("the agent crate's `familiar` feature must exist and enable nothing")
        lib = (ROOT / AGENT_CRATE / "src/lib.rs").read_text(encoding="utf-8")
        if not FAMILIAR_GATE.search(lib):
            problems.append("src/lib.rs must declare the familiar module behind its feature")
        imports = sorted(set(FAMILIAR_IMPORT.findall(code_of(ROOT / FAMILIAR_SOURCE))) - {"std", "serde_json", "super"})
        if imports:
            problems.append(f"the familiar module may import only std and serde_json, not {imports}")
    for path in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        crate = path.parent.relative_to(ROOT)
        if crate == AGENT_CRATE:
            continue
        other = tomllib.loads(path.read_text(encoding="utf-8"))
        if FAMILIAR_FEATURE in other.get("features", {}):
            problems.append(f"{crate} declares a familiar feature")
        if "sley-agent" in other.get("dependencies", {}) or "sley-agent" in other.get("build-dependencies", {}):
            problems.append(f"{crate} depends on sley-agent")
        for source in sorted((path.parent / "src").rglob("*.rs")):
            if re.search(r"\bfamiliar::", code_of(source)):
                problems.append(f"{source.relative_to(ROOT)} names the familiar module")
    return problems


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
    frontend = familiar_frontend_problems()
    record(
        "Sley source syntax or parser outside the optional authoring frontend",
        not sley_files and not parsers and not frontend,
        f"{len(sley_files)} production .sley files; parser crates in the lock: {parsers or 'none'}; "
        "benchmark fixture sources are inert corpus inputs; ADR-0055 frontend confinement: "
        + (f"problems {frontend}" if frontend else "holds (one feature-gated sley-agent module, std and serde_json only, no dependents)"),
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
        # Integration tests spawn the executable under test. They are not a
        # process surface exposed by the shipped kernel or agent binary.
        if "tests" in path.relative_to(ROOT).parts:
            continue
        source = path.read_text(encoding="utf-8", errors="ignore")
        # Inline test modules are test-binary code, like integration tests.
        if "std::process::Command" not in without_test_modules(source):
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

    passed, detail = unsafe_row(sources)
    record("unsafe code hidden in kernel", passed, detail)

    tags = [tag for tag in git("tag").split("\n") if tag]
    # The unpushed-commit count moves with every commit, so it made this
    # derived report drift at each checkpoint; scripts/check_remote_consistency.py
    # reports it at handoff instead.
    # A tag is publication: it holds only when the operator's recorded
    # publication decision names it (scripts/publication_authority.py).
    allowed_tags = set(publication_authority.authorized_tags())
    unauthorized_tags = sorted(tag for tag in tags if tag not in allowed_tags)
    record(
        "unauthorized publication/deploy/spend",
        not unauthorized_tags,
        f"git tags: {len(tags)}; outside the operator publication decision: {unauthorized_tags or 'none'}; "
        "unpushed commits are reported by scripts/check_remote_consistency.py",
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
    parser.add_argument(
        "--check-unsafe",
        action="store_true",
        help="evaluate only the unsafe-code anti-goal (a fast gate; writes nothing)",
    )
    arguments = parser.parse_args()
    if arguments.check_unsafe:
        passed, detail = unsafe_row(kernel_sources())
        print(canonical({"mode": "check-unsafe", "result": "PASS" if passed else "FAIL", "detail": detail}), end="")
        return 0 if passed else 1
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

#!/usr/bin/env python3
"""Mechanical dependency and rule audit for the S20-430 thin CLI (contract section 5)."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "crates/sley-cli"
MANIFEST = CRATE / "Cargo.toml"
PROTOCOL = ROOT / "crates/sley-protocol/src/lib.rs"
# `sley-test-runner` is admitted only for the private native-test worker
# entry (contract section 10): the production source may name it exactly
# once, as the worker call below, and nowhere else.
ALLOWED_DEPENDENCIES = {
    "sley-protocol",
    "sley-json-bridge",
    "sley-test-runner",
    "serde_json",
}
WORKER_CALL = "sley_test_runner::worker::run_input_path("
WORKER_COMMAND = '"__native-test-worker"'
ALLOWED_DEV_DEPENDENCIES = {
    "sley-repo",
    "sley-id",
    "sley-scb1",
    "sley-protocol",
    "sley-json-bridge",
    "sley-test-runner",
    "sley-vm",
    "serde_json",
}
KERNEL_CRATES = (
    "sley_ssmc",
    "sley_check",
    "sley_query",
    "sley_mutate",
    "sley_txn",
    "sley_repo",
    "sley_policy",
    "sley_vm",
    "sley_state_root",
    "sley_store",
    "sley_adapter",
    "sley_schema",
    "sley_scb1",
)


def derive_method_truth(protocol_text: str) -> tuple[tuple[int, ...], tuple[str, ...]]:
    """The structured source of operation truth: the frozen `Method` table.

    Numeric tags come from the `tag()` arms (named constants resolved from
    the same file) and dotted names from the `name()` arms, so 306/307,
    `entity.*`, and every bare method name are audited by construction and
    no future method can escape by landing outside a hand-written range.
    """
    constants = {
        name: int(value)
        for name, value in re.findall(
            r"pub const ([A-Z][A-Z0-9_]*_TAG): u32 = (\d+);", protocol_text
        )
    }
    tag_block: str
    name_block: str
    try:
        tag_block = protocol_text.split("pub const fn tag(self) -> u32", 1)[1].split(
            "pub const fn name(self)", 1
        )[0]
        name_block = protocol_text.split("pub const fn name(self)", 1)[1].split(
            "pub const fn family(self)", 1
        )[0]
    except IndexError as error:
        raise ValueError("method table regions not found") from error
    tags: set[int] = set()
    for _variant, raw in re.findall(r"Self::(\w+) => ([A-Z0-9_]+),", tag_block):
        if raw.isdigit():
            tags.add(int(raw))
        elif raw in constants:
            tags.add(constants[raw])
        else:
            raise ValueError(f"unresolvable method tag: {raw}")
    names = re.findall(r'Self::\w+ => "([\w.]+)",', name_block)
    if not tags or not names:
        raise ValueError("method table derivation is empty")
    return tuple(sorted(tags)), tuple(sorted(names))


def judgment_patterns(tags: tuple[int, ...]) -> tuple:
    """Match-arm sentinels for every frozen tag; longest numerals first so
    a three-digit tag never hides behind its prefix."""
    alternation = "|".join(sorted((str(tag) for tag in tags), key=lambda item: (-len(item), item)))
    return (
        re.compile(r"\bfn validate"),
        re.compile(r"\bfn judge"),
        re.compile(r"\bfn check_"),
        re.compile(r"Method::[A-Z][A-Za-z]+\s*(\||=>)"),
        re.compile(rf"\b({alternation})\s*=>"),
    )


def method_name_pattern(names: tuple[str, ...]) -> "re.Pattern[str]":
    """String-literal sentinels: dotted families by prefix, bare method
    names as exact literals."""
    dotted = sorted({name.split(".", 1)[0] for name in names if "." in name})
    bare = sorted({name for name in names if "." not in name})
    families = "|".join(dotted)
    literals = "|".join(re.escape(name) for name in bare)
    return re.compile(rf'"((?:{families})\.[a-z._]*|{literals})"')


ENCODE_CALL = re.compile(r"\bencode_frame(?:_for_version)?\(")


def audit_production_source(
    display: str,
    production: str,
    patterns: tuple,
    name_pattern: "re.Pattern[str]",
    problems: list,
    counters: dict,
) -> None:
    for crate in KERNEL_CRATES:
        if re.search(rf"\b{crate}\b", production):
            problems.append(f"kernel-crate:{display}:{crate}")
    if "ProtocolFailure {" in production:
        problems.append(f"failure-literal:{display}")
    counters["frame_literals"] += production.count("ProtocolFrame {")
    counters["encode_calls"] += len(ENCODE_CALL.findall(production))
    for pattern in patterns:
        if pattern.search(production):
            problems.append(f"judgment:{display}:{pattern.pattern}")
    if name_pattern.search(production):
        problems.append(f"method-name:{display}")
    # The offer carries no transport feature (contract section 2): the
    # endpoint must never name the JSON bridge feature bit.
    if "FEATURE_JSON_BRIDGE" in production:
        problems.append(f"transport-feature:{display}")
    # The worker exception is bounded (contract section 10): every mention
    # of the runner crate is the one worker call, and the private command
    # word appears once, in the parser.
    runner_mentions = len(re.findall(r"\bsley_test_runner\b", production))
    worker_calls = production.count(WORKER_CALL)
    if runner_mentions != worker_calls:
        problems.append(f"worker-edge:{display}:{runner_mentions - worker_calls}")
    counters["worker_calls"] = counters.get("worker_calls", 0) + worker_calls
    counters["worker_commands"] = counters.get("worker_commands", 0) + production.count(WORKER_COMMAND)


def worker_problems(counters: dict, problems: list) -> None:
    """Exactly one worker call and one private command word (section 10)."""
    if counters.get("worker_calls", 0) != 1:
        problems.append(f"worker-calls:{counters.get('worker_calls', 0)}")
    if counters.get("worker_commands", 0) != 1:
        problems.append(f"worker-commands:{counters.get('worker_commands', 0)}")


def section(manifest: str, name: str) -> set[str]:
    match = re.search(rf"^\[{re.escape(name)}\]\n(.*?)(?=^\[|\Z)", manifest, re.S | re.M)
    if not match:
        return set()
    return {line.split("=", 1)[0].strip() for line in match.group(1).splitlines() if "=" in line and not line.startswith("#")}


def main() -> int:
    if not CRATE.exists():
        print(json.dumps({"contract": "s20-430-cli-rule-audit-v1", "crate": "absent", "result": "PASS"}, sort_keys=True))
        return 0
    problems: list[str] = []
    manifest = MANIFEST.read_text(encoding="utf-8")
    dependencies = section(manifest, "dependencies")
    if not dependencies <= ALLOWED_DEPENDENCIES:
        problems.append(f"dependencies:{sorted(dependencies - ALLOWED_DEPENDENCIES)}")
    dev_dependencies = section(manifest, "dev-dependencies")
    if not dev_dependencies <= ALLOWED_DEV_DEPENDENCIES:
        problems.append(f"dev-dependencies:{sorted(dev_dependencies - ALLOWED_DEV_DEPENDENCIES)}")
    for other in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        if other.parent.name != "sley-cli" and "sley-cli" in other.read_text(encoding="utf-8"):
            problems.append(f"reverse-dependency:{other.parent.name}")

    sources = sorted(CRATE.glob("src/**/*.rs"))
    if not sources:
        problems.append("no-sources")
    try:
        tags, names = derive_method_truth(PROTOCOL.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(json.dumps({"contract": "s20-430-cli-rule-audit-v1", "result": "FAIL", "problems": [f"method-truth:{error}"]}, sort_keys=True))
        return 1
    patterns = judgment_patterns(tags)
    name_pattern = method_name_pattern(names)
    counters = {"frame_literals": 0, "encode_calls": 0, "worker_calls": 0, "worker_commands": 0}
    for path in sources:
        text = path.read_text(encoding="utf-8")
        # Tests live under `mod tests` or a `tests` directory and may use fixtures.
        production = text.split("#[cfg(test)]", 1)[0]
        audit_production_source(path.name, production, patterns, name_pattern, problems, counters)
    if counters["frame_literals"] > 1:
        problems.append(f"frame-literals:{counters['frame_literals']}")
    if counters["encode_calls"] > 1:
        problems.append(f"encode-frame-calls:{counters['encode_calls']}")
    worker_problems(counters, problems)

    result = {
        "contract": "s20-430-cli-rule-audit-v1",
        "crate": "present",
        "dependencies": sorted(dependencies),
        "method_tags_audited": len(tags),
        "method_names_audited": len(names),
        "frame_literals": counters["frame_literals"],
        "worker_calls": counters["worker_calls"],
        "encode_frame_calls": counters["encode_calls"],
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Build and run the S20-310 full root-backed query engine persistent libFuzzer target."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "evidence/runtime/s20-700-root-query-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/root_query_engine"
FIXTURE = ROOT / "conformance/root-backed-query/v1/accepted.json"
CLANG = "clang-18"
RUST_TOOLCHAIN = "nightly-2026-02-27"
LIBFUZZER = Path("/usr/lib/llvm-18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a")
MAX_INPUT_LEN = 4096
SMOKE_RUNS = 512
SMOKE_TIMEOUT_SECONDS = 90
QUERY_CLASSES = 19

# A minimal complete root in the target's byte grammar: one workspace (kind 1)
# whose root namespace is the single namespace (kind 3) with no members and no
# packages. Entity bytes: count, then per entity kind-1, id, fields.
COMPLETE_ROOT_PREFIX = bytes(
    [
        2,  # two entities
        0, 0x01,  # kind 1 (workspace), id 0x01
        0x02,  # root namespace 0x02
        0, 0, 0, 0,  # packages, capability requirements, contracts, tests: empty sets
        2, 0x02,  # kind 3 (namespace), id 0x02
        0,  # parent: none
        0,  # members: empty
    ]
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manual", action="store_true", help="run indefinitely until interrupted")
    parser.add_argument("--runs", type=int, default=SMOKE_RUNS)
    parser.add_argument("--timeout", type=int, default=SMOKE_TIMEOUT_SECONDS)
    args = parser.parse_args()
    if args.runs < 0 or args.timeout <= 0:
        parser.error("--runs must be nonnegative and --timeout must be positive")

    started = time.monotonic()
    RUNTIME.mkdir(parents=True, exist_ok=True)
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    corpus_count = generate_seed_corpus()

    evidence: dict[str, object] = {
        "contract": "s20-700-root-query-engine-persistent-libfuzzer-slice-v1",
        "scope": "ROOT_QUERY_ENGINE_ONLY",
        "full_s20_700_complete": False,
        "query_classes": QUERY_CLASSES,
        "max_input_bytes": MAX_INPUT_LEN,
        "corpus_count": corpus_count,
        "seed_source": str(FIXTURE.relative_to(ROOT)),
        "runtime_path": str(RUNTIME.relative_to(ROOT)),
        "commands": [],
        "problems": [],
    }

    for problem in toolchain_problems():
        evidence["problems"].append(problem)
    if evidence["problems"]:
        evidence["result"] = "BLOCKED"
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 2

    env = os.environ.copy()
    env["CC"] = CLANG
    build = run(
        [
            "cargo",
            f"+{RUST_TOOLCHAIN}",
            "rustc",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--bin",
            "root_query_engine",
            "--release",
            "--target-dir",
            str(TARGET_DIR),
            "--",
            "-Cpasses=sancov-module",
            "-Cllvm-args=-sanitizer-coverage-level=4",
            "-Cllvm-args=-sanitizer-coverage-inline-8bit-counters",
            f"-Clink-arg={LIBFUZZER}",
            "-Clink-arg=-lstdc++",
        ],
        env=env,
        timeout=args.timeout,
    )
    evidence["commands"].append(build)
    if build["returncode"] != 0:
        evidence["result"] = "FAIL"
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 1

    if args.manual:
        command = [str(FUZZER), f"-max_len={MAX_INPUT_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)]
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    fuzz = run(
        [str(FUZZER), f"-runs={args.runs}", f"-max_len={MAX_INPUT_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)],
        timeout=args.timeout,
    )
    evidence["commands"].append(fuzz)
    evidence["libfuzzer_output_tail"] = (fuzz["stderr"] + fuzz["stdout"])[-4000:]
    evidence["duration_seconds"] = round(time.monotonic() - started, 3)
    evidence["result"] = "PASS" if fuzz["returncode"] == 0 else "FAIL"
    EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 0 if evidence["result"] == "PASS" else 1


def toolchain_problems() -> list[str]:
    problems = []
    if shutil.which(CLANG) is None:
        problems.append(f"{CLANG}-missing")
    if not LIBFUZZER.exists():
        problems.append(f"libfuzzer-runtime-missing:{LIBFUZZER}")
    rustup = shutil.which("rustup")
    if rustup is None:
        problems.append("rustup-missing")
    else:
        result = subprocess.run(
            ["rustup", "toolchain", "list"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if RUST_TOOLCHAIN not in result.stdout:
            problems.append(f"rust-toolchain-missing:{RUST_TOOLCHAIN}")
    return problems


def generate_seed_corpus() -> int:
    if CORPUS.exists():
        shutil.rmtree(CORPUS)
    CORPUS.mkdir(parents=True)

    accepted = json.loads(FIXTURE.read_text(encoding="utf-8"))
    if accepted.get("contract") != "sley2-root-backed-query-v1":
        raise SystemExit("root-backed query fixture contract drifted")
    if accepted.get("query_classes") != QUERY_CLASSES:
        raise SystemExit("root-backed query fixture class count drifted")

    seeds: list[bytes] = []
    # Every class over the minimal complete root, with and without
    # continuation, over a spread of limits and cursors.
    for cls in range(QUERY_CLASSES):
        for allow in (0, 1):
            for limit_byte in (0, 1, 3):
                for cursor in (0, 1, 2, 3):
                    query = bytes([cls, 1, 4, 0x8F, 0x07, 2, 0, 1, limit_byte, limit_byte, 0, 0, 0, allow, cursor, 1, 2, 3])
                    seeds.append(COMPLETE_ROOT_PREFIX + query)
    # Raw byte spreads exercise the root decoder and judgment rejections.
    seeds.extend(bytes([value]) for value in range(256))
    for value in range(256):
        length = 2 + (value % 61)
        seeds.append(bytes((value + (offset * 29)) % 256 for offset in range(length)))
    seeds.extend([bytes(range(128)), bytes(reversed(range(128))), bytes([0xFF]) * 128])

    unique_seeds = list(dict.fromkeys(seeds))
    for index, seed in enumerate(unique_seeds):
        digest = hashlib.sha256(seed).hexdigest()[:16]
        (CORPUS / f"seed-{index:04d}-{digest}").write_bytes(seed)
    return len(unique_seeds)


def run(command: list[str], *, env: dict[str, str] | None = None, timeout: int) -> dict[str, object]:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        return {
            "argv": command,
            "returncode": completed.returncode,
            "duration_seconds": round(time.monotonic() - started, 3),
            "stdout": completed.stdout[-4000:],
            "stderr": completed.stderr[-4000:],
        }
    except subprocess.TimeoutExpired as error:
        return {
            "argv": command,
            "returncode": 124,
            "duration_seconds": round(time.monotonic() - started, 3),
            "stdout": output_tail(error.stdout),
            "stderr": output_tail(error.stderr),
            "timeout_seconds": timeout,
        }


def output_tail(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        value = value.decode("utf-8", errors="replace")
    return value[-4000:]


if __name__ == "__main__":
    sys.exit(main())

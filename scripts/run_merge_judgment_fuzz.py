#!/usr/bin/env python3
"""Build and run the S20-520 merge-judgment persistent libFuzzer target.

The target drives `sley_repo::judge_merge` with byte-decoded mutation scripts
over one fixed valid base root (see `fuzz/targets/merge_judgment.rs`). Seeds
are deterministic scripts: every single-mutation script on each side plus a
handful of two-sided combinations and the empty script.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "evidence/runtime/s20-700-merge-judgment-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/merge_judgment"
CLANG = "clang-18"
RUST_TOOLCHAIN = "nightly-2026-02-27"
LIBFUZZER = Path("/usr/lib/llvm-18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a")
MAX_LEN = 34
SMOKE_RUNS = 512
SMOKE_TIMEOUT_SECONDS = 120
SLOTS = (4, 6, 16, 18, 19)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
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
        "contract": "s20-700-merge-judgment-libfuzzer-slice-v1",
        "scope": "MERGE_JUDGMENT_OUTCOME_AND_DETERMINISM",
        "section_18_5_surface": "merge engine",
        "full_s20_700_complete": False,
        "max_script_bytes": MAX_LEN,
        "corpus_count": corpus_count,
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
            "merge_judgment",
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
        command = [str(FUZZER), f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)]
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    fuzz = run(
        [str(FUZZER), f"-runs={args.runs}", f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)],
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
    seeds: list[bytes] = [b"\x00"]
    for side in (0x00, 0x80):
        for slot in SLOTS:
            for mutation in range(4):
                seeds.append(bytes([0x00, side | slot, mutation]))
    # Two-sided combinations: disjoint fields, convergent, and conflicting.
    seeds.append(bytes([0x00, 6, 0, 0x80 | 19, 0]))
    seeds.append(bytes([0x00, 6, 0, 0x80 | 6, 0]))
    seeds.append(bytes([0x00, 18, 0, 0x80 | 18, 0]))
    seeds.append(bytes([0x00, 18, 0, 0x80 | 19, 0]))
    seeds.append(bytes([0x00, 6, 1, 0x80 | 6, 0]))
    seeds.append(bytes([0x00, 6, 1, 0x80 | 6, 1]))
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
            check=False,
            timeout=timeout,
        )
        return {
            "argv": command,
            "returncode": completed.returncode,
            "stdout": completed.stdout[-2000:],
            "stderr": completed.stderr[-2000:],
            "duration_seconds": round(time.monotonic() - started, 3),
        }
    except subprocess.TimeoutExpired:
        return {
            "argv": command,
            "returncode": "timeout",
            "stdout": "",
            "stderr": f"exceeded {timeout}s",
            "duration_seconds": round(time.monotonic() - started, 3),
        }


if __name__ == "__main__":
    raise SystemExit(main())

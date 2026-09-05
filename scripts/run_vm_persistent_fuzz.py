#!/usr/bin/env python3
"""Build and run the scoped restricted-VM canonical-input fuzz target."""

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
RUNTIME = ROOT / "evidence/runtime/s20-700-vm-input-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/vm_canonical_inputs"
CLANG = "clang-18"
RUST_TOOLCHAIN = "nightly-2026-02-27"
LIBFUZZER = Path("/usr/lib/llvm-18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a")
MAX_INPUT_LEN = 4096
SMOKE_RUNS = 1024
SMOKE_TIMEOUT_SECONDS = 120
FIXTURE_COUNT = 9
# Extended-profile fixtures, one per landed opcode family beyond E1.
EXTENDED_FIXTURE_COUNT = 8


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
    reset_directory(ARTIFACTS)
    corpus_count = generate_seed_corpus()
    # A run shorter than the corpus replays its prefix instead of smoking it:
    # libFuzzer works through the seed files first, so family lanes past the
    # cutoff would never execute yet still report PASS.
    if not args.manual and args.runs < corpus_count:
        parser.error(f"--runs ({args.runs}) must cover the corpus ({corpus_count})")
    evidence: dict[str, object] = {
        "contract": "s20-700-vm-canonical-inputs-persistent-libfuzzer-slice-v1",
        "scope": "VM_INPUT_EXTENDED_FAMILY_S20_700_BOUNDARY",
        "runs_requested": args.runs,
        "full_s20_700_complete": False,
        "full_s20_270_complete": False,
        "raw_bytecode_decoder_claimed": False,
        "raw_bytecode_execution_entrypoint_claimed": False,
        "fixture_count": FIXTURE_COUNT,
        "extended_family_fixture_count": EXTENDED_FIXTURE_COUNT,
        "identity_fixture_count": 6,
        "boolean_opcode_fixture_count": 3,
        "max_input_bytes": MAX_INPUT_LEN,
        "max_raw_inputs": 4,
        "max_collection_items": 4,
        "max_payload_bytes": 32,
        "corpus_count": corpus_count,
        "synthetic_seed_grammar": "bounded fuzz-only typed VM input constructors v1",
        "runtime_path": str(RUNTIME.relative_to(ROOT)),
        "source_commit": git_output(["git", "rev-parse", "HEAD"]),
        "worktree_dirty": bool(git_output(["git", "status", "--porcelain"])),
        "commands": [],
        "problems": toolchain_problems(),
    }
    if evidence["problems"]:
        evidence["result"] = "BLOCKED"
        write_evidence(evidence)
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
            "vm_canonical_inputs",
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
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
        write_evidence(evidence)
        return 1

    if args.manual:
        command = fuzzer_command(runs=None)
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    fuzz = run(fuzzer_command(runs=args.runs), timeout=args.timeout)
    evidence["commands"].append(fuzz)
    evidence["libfuzzer_output_tail"] = (fuzz["stderr"] + fuzz["stdout"])[-4000:]
    evidence["duration_seconds"] = round(time.monotonic() - started, 3)
    evidence["executed_runs"] = parse_executed_units(str(fuzz["stderr"]))
    if fuzz["returncode"] == 0 and evidence["executed_runs"] >= corpus_count:
        evidence["result"] = "PASS"
    else:
        evidence["result"] = "FAIL"
        evidence["problems"] = [
            f"executed {evidence['executed_runs']} of {corpus_count} corpus seeds"
        ]
    write_evidence(evidence)
    return 0 if evidence["result"] == "PASS" else 1


def parse_executed_units(stderr: str) -> int:
    """Units libFuzzer reports executing, or -1 when the line is absent."""
    import re

    matches = re.findall(r"Done (\d+) runs", stderr)
    return int(matches[-1]) if matches else -1


def fuzzer_command(*, runs: int | None) -> list[str]:
    command = [
        str(FUZZER),
        f"-max_len={MAX_INPUT_LEN}",
        f"-artifact_prefix={ARTIFACTS}/",
    ]
    if runs is not None:
        command.append(f"-runs={runs}")
    command.append(str(CORPUS))
    return command


def generate_seed_corpus() -> int:
    seeds = [bytes([value]) for value in range(256)]
    for value in range(256):
        length = 2 + (value % 31)
        seeds.append(bytes((value + (offset * 37)) % 256 for offset in range(length)))

    # The two prefix bytes are the fixture and the family gate (0 fires); the
    # repeated limit selector fills the remaining header, so each seed pins a
    # different lane combination and limit profile by construction.
    for fixture in range(FIXTURE_COUNT):
        for limit_selector in range(6):
            seeds.append(bytes([fixture, 0]) + bytes([limit_selector]) * 64)
            seeds.append(bytes([fixture, 1]) + bytes([limit_selector]) * 64)

    # Lane decisions sit at fixed offsets consumed before any variable-length
    # value construction, so every seed byte selects the lane it names no
    # matter how many bytes the outer fixture's canonical value consumes:
    # [fixture, family_gate, family, extended_toggle, map_toggle,
    #  canonical_flag, limit_selector] + filler.
    for fixture in range(FIXTURE_COUNT):
        for family in range(EXTENDED_FIXTURE_COUNT):
            seeds.append(bytes([fixture, 0, family, 1, 0, 0, 0]) + bytes([0]) * 64)
            seeds.append(bytes([fixture, 0, family, 0, 1, 1, 3]) + bytes([0xFF]) * 64)

    seeds.extend(
        [
            bytes([0, 0, 0, 0]),
            bytes([8, 0, 1, 1, 0]),
            bytes(range(64)),
            bytes(reversed(range(64))),
            bytes([0xFF]) * 64,
        ]
    )
    reset_directory(CORPUS)
    unique_seeds = list(dict.fromkeys(seeds))
    for index, seed in enumerate(unique_seeds):
        digest = hashlib.sha256(seed).hexdigest()[:16]
        (CORPUS / f"seed-{index:04d}-{digest}").write_bytes(seed)
    return len(unique_seeds)


def reset_directory(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True)


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


def git_output(command: list[str]) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else "UNAVAILABLE"


def run(
    command: list[str], *, env: dict[str, str] | None = None, timeout: int
) -> dict[str, object]:
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


def write_evidence(evidence: dict[str, object]) -> None:
    EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print(json.dumps(evidence, indent=2, sort_keys=True))


if __name__ == "__main__":
    sys.exit(main())

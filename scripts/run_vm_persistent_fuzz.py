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
# Owner-lane toolchain resolution (repair round 7): the pinned clang-18 and
# libfuzzer paths above stay the qualification defaults. SLEY_FUZZ_CC and
# SLEY_FUZZ_LIBFUZZER_A let an owner lane prove the harness on an equivalent
# toolchain without editing it; the resolved values land in evidence.json.
CC = os.environ.get("SLEY_FUZZ_CC", CLANG)
FUZZER_RT = Path(os.environ.get("SLEY_FUZZ_LIBFUZZER_A", str(LIBFUZZER)))
BUILD_TIMEOUT_SECONDS = 1200
MAX_INPUT_LEN = 4096
SMOKE_RUNS = 1576
SMOKE_TIMEOUT_SECONDS = 120
FIXTURE_COUNT = 9
# Extended-profile fixtures, one per landed opcode family beyond E1
# (E8 bridge names family selector 8; its tamper/budget subl lane consumes
# cursor bytes after the shared family lane, leaving header offsets stable).
EXTENDED_FIXTURE_COUNT = 9


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manual", action="store_true", help="run indefinitely until interrupted")
    parser.add_argument("--runs", type=int, default=SMOKE_RUNS)
    parser.add_argument("--timeout", type=int, default=SMOKE_TIMEOUT_SECONDS)
    parser.add_argument("--build-timeout", type=int, default=BUILD_TIMEOUT_SECONDS)
    args = parser.parse_args()
    if args.runs < 0 or args.timeout <= 0 or args.build_timeout <= 0:
        parser.error("--runs must be nonnegative and --timeout must be positive")

    started = time.monotonic()
    RUNTIME.mkdir(parents=True, exist_ok=True)
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    corpus_count, stale_seeds_removed = generate_seed_corpus()
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
    # Repair round 7 durable harness provenance (uniform across slices).
    evidence.setdefault("runs_requested", args.runs)
    evidence.setdefault("build_locked", True)
    evidence.setdefault("corpus_persistent", True)
    evidence.setdefault("sancov_scope", "workspace-target-units-via-host-config")
    evidence.setdefault("toolchain_overridden", CC != CLANG or FUZZER_RT != LIBFUZZER)
    evidence.setdefault("cc", CC)
    evidence.setdefault("libfuzzer_runtime", str(FUZZER_RT))
    evidence.setdefault("stale_seeds_removed", stale_seeds_removed)
    if evidence["problems"]:
        evidence["result"] = "BLOCKED"
        write_evidence(evidence)
        return 2

    env = os.environ.copy()
    env["CC"] = CC
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
            "--locked",
            "--target-dir",
            str(TARGET_DIR),
            "-Ztarget-applies-to-host",
            "-Zhost-config",
            "--config",
            "host.rustflags=[]",
            "--config",
            "target.x86_64-unknown-linux-gnu.rustflags=[\"-Cpasses=sancov-module\", \"-Cllvm-args=-sanitizer-coverage-level=4\", \"-Cllvm-args=-sanitizer-coverage-inline-8bit-counters\"]",
            "--",
            "-Cpasses=sancov-module",
            "-Cllvm-args=-sanitizer-coverage-level=4",
            "-Cllvm-args=-sanitizer-coverage-inline-8bit-counters",
            f"-Clink-arg={FUZZER_RT}",
            "-Clink-arg=-lstdc++",
        ],
        env=env,
        timeout=args.build_timeout,
    )
    evidence["commands"].append(build)
    if build["returncode"] != 0:
        evidence["result"] = "FAIL"
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
        write_evidence(evidence)
        return 1

    evidence["owner_lib_sancov_symbols"] = owner_lib_sancov_symbols()
    evidence["owner_lib_sancov_total"] = sum(evidence["owner_lib_sancov_symbols"].values())
    if evidence["owner_lib_sancov_total"] == 0:
        evidence["result"] = "FAIL"
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
        evidence.setdefault("problems", []).append("owner-lib-sancov-missing")
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
    evidence["coverage_feedback_observed"] = "ft:" in str(fuzz["stderr"] + fuzz["stdout"])
    evidence["crash_artifacts"] = crash_artifact_names(ARTIFACTS)
    evidence["minimized_crashes"] = (
        minimize_crashes(
            fuzzer_bin=str(FUZZER),
            artifacts_dir=ARTIFACTS,
            corpus_dir=CORPUS,
            minimized_dir=RUNTIME / "minimized",
            timeout_seconds=args.timeout,
        )
        if evidence["crash_artifacts"]
        else []
    )
    if (
        fuzz["returncode"] == 0
        and evidence["executed_runs"] >= corpus_count
        and evidence["coverage_feedback_observed"]
        and not evidence["crash_artifacts"]
    ):
        evidence["result"] = "PASS"
    else:
        evidence["result"] = "FAIL"
        evidence["problems"] = [
            f"executed {evidence['executed_runs']} of {corpus_count} corpus seeds "
            f"(coverage_feedback={evidence['coverage_feedback_observed']}, "
            f"crashes={evidence['crash_artifacts']})"
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


def generate_seed_corpus() -> tuple[int, int]:
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
            # Minimized lane regression (RW-050 slice 2): a no-op tamper
            # class once let a frozen V2B1 row through as "tampered". The
            # fixed lane asserts every tampered row differs from frozen.
            bytes([0x20, 0x45, 0x6B]),
        ]
    )
    CORPUS.mkdir(parents=True, exist_ok=True)
    unique_seeds = list(dict.fromkeys(seeds))
    for index, seed in enumerate(unique_seeds):
        digest = hashlib.sha256(seed).hexdigest()[:16]
        (CORPUS / f"seed-{index:04d}-{digest}").write_bytes(seed)
    written = {
        f"seed-{index:04d}-{hashlib.sha256(seed).hexdigest()[:16]}"
        for index, seed in enumerate(unique_seeds)
    }
    stale_seeds_removed = sync_seed_corpus(CORPUS, written)
    return len(unique_seeds), stale_seeds_removed



def sync_seed_corpus(corpus_dir: Path, written: set[str]) -> int:
    """Drop stale seed-* files from prior seed sets without wiping the corpus.

    Repair round 7: the corpus directory persists across runs so
    libFuzzer-added inputs survive; only deterministic seeds absent from the
    current set are removed. Crash artifacts live under the artifacts dir,
    never here.
    """
    removed = 0
    for entry in sorted(corpus_dir.iterdir()):
        if entry.is_file() and entry.name.startswith("seed-") and entry.name not in written:
            entry.unlink()
            removed += 1
    return removed


def crash_artifact_names(artifacts_dir: Path) -> list[str]:
    """Crash-class files libFuzzer left under the artifacts dir, if any."""
    if not artifacts_dir.exists():
        return []
    return sorted(
        entry.name
        for entry in artifacts_dir.iterdir()
        if entry.is_file() and entry.name.startswith(("crash-", "oom-", "timeout-", "leak-"))
    )


def minimize_crashes(
    *,
    fuzzer_bin: str,
    artifacts_dir: Path,
    corpus_dir: Path,
    minimized_dir: Path,
    timeout_seconds: int,
) -> list[dict[str, object]]:
    """Minimize each crasher via libFuzzer -merge=1.

    The minimized input lands under ARTIFACTS/minimized-* next to the
    original. Filing it under fuzz/regressions/ stays an explicit owner
    step, never an automatic commit from a smoke run.
    """
    minimized_dir.mkdir(parents=True, exist_ok=True)
    out: list[dict[str, object]] = []
    for name in crash_artifact_names(artifacts_dir):
        source = artifacts_dir / name
        try:
            completed = subprocess.run(
                [fuzzer_bin, "-merge=1", str(minimized_dir), str(corpus_dir), str(source)],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=timeout_seconds,
                check=False,
            )
            record: dict[str, object] = {
                "artifact": name,
                "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "size_bytes": source.stat().st_size,
                "merge_returncode": completed.returncode,
                "merge_tail": (completed.stderr + completed.stdout)[-2000:],
            }
            minimized = [p for p in sorted(minimized_dir.iterdir()) if p.is_file()]
            if minimized:
                smallest = min(minimized, key=lambda p: p.stat().st_size)
                record["minimized_sha256"] = hashlib.sha256(smallest.read_bytes()).hexdigest()
                record["minimized_size_bytes"] = smallest.stat().st_size
                (artifacts_dir / f"minimized-{record['minimized_sha256']}").write_bytes(
                    smallest.read_bytes()
                )
            out.append(record)
        except (OSError, subprocess.TimeoutExpired) as error:
            out.append({"artifact": name, "minimize_error": str(error)[:500]})
    return out


def owner_lib_sancov_symbols() -> dict[str, int]:
    """sanitizer_cov symbol counts per sley-* rlib in this slice's target dir.

    Proof that coverage instrumentation reaches the owner library code, not
    just the fuzz binary crate: with the old bin-only -Cpasses flag the
    owner rlibs carry zero sanitizer_cov symbols.
    """
    nm = shutil.which("llvm-nm") or shutil.which("nm")
    counts: dict[str, int] = {}
    if nm is None:
        return counts
    for rlib in sorted((TARGET_DIR / "release" / "deps").glob("libsley_*.rlib")):
        try:
            completed = subprocess.run(
                [nm, str(rlib)],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=300,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            continue
        counts[rlib.name] = sum(
            1 for line in completed.stdout.splitlines() if "sanitizer_cov" in line
        )
    return counts


def toolchain_problems() -> list[str]:
    problems = []
    if shutil.which(CC) is None:
        problems.append(f"{CC}-missing")
    if not FUZZER_RT.exists():
        problems.append(f"libfuzzer-runtime-missing:{FUZZER_RT}")
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

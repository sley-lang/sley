#!/usr/bin/env python3
"""Build and run the S20-320 full context capsule builder persistent libFuzzer target."""

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
RUNTIME = ROOT / "evidence/runtime/s20-700-context-capsule-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/context_capsule_builder"
FIXTURE = ROOT / "conformance/context-capsule/v1/accepted.json"
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
SMOKE_RUNS = 1942
SMOKE_TIMEOUT_SECONDS = 90
QUERY_CLASSES = 19

# A minimal complete root in the target's byte grammar: one workspace whose
# root namespace is the single namespace with no members and no packages.
COMPLETE_ROOT_PREFIX = bytes(
    [
        2,
        0, 0x01,
        0x02,
        0, 0, 0, 0,
        2, 0x02,
        0,
        0,
    ]
)


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
    # libFuzzer works through the seed files first, so lanes past the cutoff
    # would never execute yet still report PASS.
    if not args.manual and args.runs < corpus_count:
        parser.error(f"--runs ({args.runs}) must cover the corpus ({corpus_count})")

    evidence: dict[str, object] = {
        "contract": "s20-700-context-capsule-builder-persistent-libfuzzer-slice-v1",
        "scope": "CONTEXT_CAPSULE_BUILDER_ONLY",
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
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
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
            "context_capsule_builder",
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
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 1

    evidence["owner_lib_sancov_symbols"] = owner_lib_sancov_symbols()
    evidence["owner_lib_sancov_total"] = sum(evidence["owner_lib_sancov_symbols"].values())
    if evidence["owner_lib_sancov_total"] == 0:
        evidence["result"] = "FAIL"
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
        evidence.setdefault("problems", []).append("owner-lib-sancov-missing")
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
        evidence.setdefault("problems", []).append(
            f"executed {evidence['executed_runs']} of {corpus_count} corpus seeds "
            f"(coverage_feedback={evidence['coverage_feedback_observed']}, "
            f"crashes={evidence['crash_artifacts']})"
        )
    EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 0 if evidence["result"] == "PASS" else 1


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


def parse_executed_units(stderr: str) -> int:
    """Units libFuzzer reports executing, or -1 when the line is absent."""
    import re

    matches = re.findall(r"Done (\d+) runs", stderr)
    return int(matches[-1]) if matches else -1


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


def generate_seed_corpus() -> tuple[int, int]:
    CORPUS.mkdir(parents=True, exist_ok=True)

    accepted = json.loads(FIXTURE.read_text(encoding="utf-8"))
    if accepted.get("contract") != "sley2-context-capsule-v1":
        raise SystemExit("context capsule fixture contract drifted")
    if len(accepted.get("vectors", [])) != 24:
        raise SystemExit("context capsule fixture vector count drifted")

    seeds: list[bytes] = []
    for cls in range(QUERY_CLASSES):
        for allow in (0, 1):
            for limit_byte in (0, 1, 3):
                for cursor in (0, 1, 2, 3):
                    query = bytes([cls, 1, 4, 0x8F, 0x07, 2, 0, 1, limit_byte, limit_byte, 0, 0, 0, allow, cursor, 1, 2, 3])
                    seeds.append(COMPLETE_ROOT_PREFIX + query)
    seeds.extend(bytes([value]) for value in range(256))
    for value in range(256):
        length = 2 + (value % 61)
        seeds.append(bytes((value + (offset * 31)) % 256 for offset in range(length)))
    seeds.extend([bytes(range(128)), bytes(reversed(range(128))), bytes([0xFF]) * 128])

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

#!/usr/bin/env python3
"""Build and run scoped persistent fuzz targets for public semantic checkers."""

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
RUNTIME = ROOT / "evidence/runtime/s20-700-semantic-checkers-libfuzzer"
TARGET_DIR = RUNTIME / "target"
EVIDENCE = RUNTIME / "evidence.json"
HARNESS_REGRESSION = ROOT / "fuzz/regressions/S20_700_HARNESS_001.json"
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
OWNER_RLIB = "libsley_check-"
MAX_INPUT_LEN = 4096
SMOKE_RUNS = 792
SMOKE_TIMEOUT_SECONDS = 60
TARGETS = {
    "type-checker": {
        "binary": "type_checker",
        "scope": "S20_210_PUBLIC_TYPED_TYPE_CHECKER",
        "corpus": RUNTIME / "type-checker-corpus",
        "artifacts": RUNTIME / "type-checker-artifacts",
    },
    "graph-cfg": {
        "binary": "ssmc_graph_cfg_checker",
        "scope": "S20_220_PUBLIC_TYPED_GRAPH_CFG_CHECKER",
        "corpus": RUNTIME / "graph-cfg-corpus",
        "artifacts": RUNTIME / "graph-cfg-artifacts",
    },
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manual", action="store_true", help="run one target until interrupted")
    parser.add_argument(
        "--target",
        choices=["all", *TARGETS],
        default="all",
        help="target to run; smoke mode defaults to both",
    )
    parser.add_argument("--runs", type=int, default=SMOKE_RUNS)
    parser.add_argument("--timeout", type=int, default=SMOKE_TIMEOUT_SECONDS)
    parser.add_argument("--build-timeout", type=int, default=BUILD_TIMEOUT_SECONDS)
    args = parser.parse_args()
    if args.runs < 0 or args.timeout <= 0 or args.build_timeout <= 0:
        parser.error("--runs must be nonnegative and --timeout must be positive")
    if args.manual and args.target == "all":
        parser.error("--manual requires --target type-checker or --target graph-cfg")

    selected = list(TARGETS) if args.target == "all" else [args.target]
    started = time.monotonic()
    RUNTIME.mkdir(parents=True, exist_ok=True)
    corpus_counts = {}
    stale_seeds_removed = {}
    for target_name, generator in (
        ("type-checker", generate_type_checker_corpus),
        ("graph-cfg", generate_graph_cfg_corpus),
    ):
        corpus_counts[target_name], stale_seeds_removed[target_name] = generator()
    # The coverage floor is measured on the on-disk corpus files: libFuzzer
    # adds inputs every run, so the floor (files plus 256) keeps every
    # smoke covering its target corpus with headroom for genuine mutation.
    corpus_file_counts = {
        target_name: sum(1 for entry in TARGETS[target_name]["corpus"].iterdir() if entry.is_file())
        for target_name in selected
    }
    runs_floor = max(args.runs, max(corpus_file_counts.values()) + 256)
    for target in TARGETS.values():
        target["artifacts"].mkdir(parents=True, exist_ok=True)

    evidence: dict[str, object] = {
        "contract": "s20-700-semantic-checkers-persistent-libfuzzer-slice-v1",
        "scope": "PUBLIC_TYPED_S20_210_AND_S20_220_CHECKERS_ONLY",
        "full_s20_700_complete": False,
        "canonical_graph_decoder_claimed": False,
        "private_mutation_codec_used": False,
        "max_input_bytes": MAX_INPUT_LEN,
        "selected_targets": selected,
        "synthetic_seed_grammar": "bounded fuzz-only typed constructors v1",
        "development_regression_fixture": str(HARNESS_REGRESSION.relative_to(ROOT)),
        "source_commit": git_output(["git", "rev-parse", "HEAD"]),
        "worktree_dirty": bool(git_output(["git", "status", "--porcelain"])),
        "targets": {},
        "commands": [],
        "problems": toolchain_problems(),
    }
    for name in selected:
        evidence["targets"][name] = {
            "scope": TARGETS[name]["scope"],
            "corpus_count": corpus_counts[name],
            "corpus_path": str(TARGETS[name]["corpus"].relative_to(ROOT)),
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
    evidence.setdefault("toolchain_versions", toolchain_versions())
    if evidence["problems"]:
        evidence["result"] = "BLOCKED"
        write_evidence(evidence)
        return 2

    env = os.environ.copy()
    env["CC"] = CC
    for name in selected:
        binary = str(TARGETS[name]["binary"])
        build = run(
            [
                "cargo",
                f"+{RUST_TOOLCHAIN}",
                "rustc",
                "--manifest-path",
                "fuzz/Cargo.toml",
                "--bin",
                binary,
                "--release",
            "--locked",
                "--target-dir",
                str(TARGET_DIR),
                "-Ztarget-applies-to-host",
                "-Zhost-config",
                "--config",
                "host.rustflags=[]",
                "--config",
                "target.x86_64-unknown-linux-gnu.rustflags=[\"-Cpasses=sancov-module\", \"-Cllvm-args=-sanitizer-coverage-level=4\", \"-Cllvm-args=-sanitizer-coverage-inline-8bit-counters\", \"-Cllvm-args=-sanitizer-coverage-trace-compares\", \"-Cllvm-args=-sanitizer-coverage-pc-table\"]",
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
        evidence["targets"][name]["build_result"] = (
            "PASS" if build["returncode"] == 0 else "FAIL"
        )
        if build["returncode"] != 0:
            evidence["result"] = "FAIL"
            evidence["duration_seconds"] = round(time.monotonic() - started, 3)
            write_evidence(evidence)
            return 1

    evidence["owner_lib_sancov_symbols"] = owner_lib_sancov_symbols()
    evidence["owner_lib_sancov_total"] = sum(evidence["owner_lib_sancov_symbols"].values())
    evidence["owner_lib_sancov"] = sum(
        count
        for name, count in evidence["owner_lib_sancov_symbols"].items()
        if name.startswith(OWNER_RLIB)
    )
    if evidence["owner_lib_sancov_total"] == 0 or evidence["owner_lib_sancov"] == 0:
        evidence["result"] = "FAIL"
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
        evidence.setdefault("problems", []).append(f"owner-lib-sancov-missing:{OWNER_RLIB}")
        write_evidence(evidence)
        return 1

    if args.manual:
        name = selected[0]
        command = fuzzer_command(name, runs=None)
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    failed = False
    for name in selected:
        target = TARGETS[name]
        corpus_count = corpus_counts[name]
        fuzz = run(fuzzer_command(name, runs=runs_floor), timeout=args.timeout)
        evidence["commands"].append(fuzz)
        evidence["targets"][name]["fuzz_result"] = (
            "PASS" if fuzz["returncode"] == 0 else "FAIL"
        )
        evidence["targets"][name]["libfuzzer_output_tail"] = (
            fuzz["stderr"] + fuzz["stdout"]
        )[-4000:]
        fuzz_output = str(fuzz["stderr"] + fuzz["stdout"])
        evidence["targets"][name]["executed_runs"] = parse_executed_units(str(fuzz["stderr"]))
        evidence["targets"][name]["runs_floor"] = runs_floor
        evidence["targets"][name]["corpus_file_count"] = corpus_file_counts[name]
        evidence["targets"][name]["coverage"] = parse_coverage(fuzz_output)
        coverage_counters = evidence["targets"][name]["coverage"]["counters"]
        evidence["targets"][name]["coverage_ok"] = (
            coverage_counters > 0
            if isinstance(coverage_counters, int)
            else ("ft:" in fuzz_output)
        )
        evidence["targets"][name]["crash_artifacts"] = crash_artifact_names(target["artifacts"])
        evidence["targets"][name]["minimized_crashes"] = (
            minimize_crashes(
                fuzzer_bin=str(TARGET_DIR / "release" / str(target["binary"])),
                artifacts_dir=target["artifacts"],
                minimized_dir=RUNTIME / f"minimized-{name}",
                timeout_seconds=args.timeout,
            )
            if evidence["targets"][name]["crash_artifacts"]
            else []
        )
        evidence["targets"][name]["stale_seeds_removed"] = stale_seeds_removed[name]
        evidence["targets"][name]["unexpected_warnings"] = unexpected_warnings(fuzz_output)
        if (
            fuzz["returncode"] != 0
            or evidence["targets"][name]["executed_runs"] < runs_floor
            or not evidence["targets"][name]["coverage_ok"]
            or evidence["targets"][name]["crash_artifacts"]
            or evidence["targets"][name]["unexpected_warnings"]
        ):
            evidence["targets"][name]["fuzz_result"] = "FAIL"
            evidence.setdefault("problems", []).append(
                f"{name}: executed {evidence['targets'][name]['executed_runs']} of "
                f"floor {runs_floor} "
                f"(coverage={evidence['targets'][name]['coverage']}, "
                f"crashes={evidence['targets'][name]['crash_artifacts']}, "
                f"warnings={evidence['targets'][name]['unexpected_warnings']})"
            )
        failed = failed or evidence["targets"][name]["fuzz_result"] != "PASS"

    evidence["duration_seconds"] = round(time.monotonic() - started, 3)
    evidence["result"] = "FAIL" if failed else "PASS"
    write_evidence(evidence)
    return 1 if failed else 0


def fuzzer_command(name: str, *, runs: int | None) -> list[str]:
    target = TARGETS[name]
    command = [
        str(TARGET_DIR / "release" / str(target["binary"])),
        f"-max_len={MAX_INPUT_LEN}",
        f"-artifact_prefix={target['artifacts']}/",
    ]
    if runs is not None:
        command.append(f"-runs={runs}")
        command.append("-len_control=0")
    command.append(str(target["corpus"]))
    return command


def generate_type_checker_corpus() -> tuple[int, int]:
    seeds = [bytes([value]) for value in range(256)]
    regression = json.loads(HARNESS_REGRESSION.read_text(encoding="utf-8"))
    if regression.get("finding_id") != "S20-700-HARNESS-001":
        raise SystemExit("semantic-checker harness regression fixture drifted")
    seeds.append(bytes.fromhex(regression["input_hex"]))
    for value in range(128):
        seeds.append(
            bytes((value + (offset * 17)) % 256 for offset in range(1 + (value % 31)))
        )
    seeds.extend(
        [
            bytes([0, 0, 0, 0]),
            bytes([1, 0, 1, 0, 1, 0, 1, 0]),
            bytes([8, 4, 3, 2, 1, 0, 19, 18, 17, 16]),
            bytes(range(64)),
            bytes(reversed(range(64))),
            bytes([0xFF]) * 64,
        ]
    )
    return write_corpus(TARGETS["type-checker"]["corpus"], seeds)


def generate_graph_cfg_corpus() -> tuple[int, int]:
    seeds = [bytes([template, 0]) for template in range(4)]
    for template in range(4):
        for mutation in range(33):
            for argument in (0, 0xFF):
                seeds.append(bytes([template, 1, mutation, argument, mutation ^ argument]))
        for offset in range(32):
            seeds.append(
                bytes(
                    [
                        template,
                        4,
                        offset % 33,
                        (offset + 7) % 33,
                        (offset + 13) % 33,
                        (offset + 23) % 33,
                        offset,
                        0xFF - offset,
                    ]
                )
            )
    return write_corpus(TARGETS["graph-cfg"]["corpus"], seeds)


def write_corpus(path: Path, seeds: list[bytes]) -> tuple[int, int]:
    path.mkdir(parents=True, exist_ok=True)
    unique_seeds = list(dict.fromkeys(seeds))
    for index, seed in enumerate(unique_seeds):
        digest = hashlib.sha256(seed).hexdigest()[:16]
        (path / f"seed-{index:04d}-{digest}").write_bytes(seed)
    written = {
        f"seed-{index:04d}-{hashlib.sha256(seed).hexdigest()[:16]}"
        for index, seed in enumerate(unique_seeds)
    }
    return len(unique_seeds), sync_seed_corpus(path, written)


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
    minimized_dir: Path,
    timeout_seconds: int,
) -> list[dict[str, object]]:
    """Minimize each crasher with libFuzzer -minimize_crash=1.

    Repair round 7c: -merge=1 merges corpus directories and its
    crash-resistant driver skips crashing inputs, so it cannot minimize
    a crasher; -minimize_crash=1 shrinks the single crashing input and
    writes the exact minimized artifact. Filing the minimized input
    under fuzz/regressions/ stays an explicit owner step, never an
    automatic commit from a smoke run.
    """
    minimized_dir.mkdir(parents=True, exist_ok=True)
    out: list[dict[str, object]] = []
    for name in crash_artifact_names(artifacts_dir):
        source = artifacts_dir / name
        record: dict[str, object] = {
            "artifact": name,
            "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            "size_bytes": source.stat().st_size,
        }
        exact = minimized_dir / f"minimized-{name}"
        try:
            completed = subprocess.run(
                [
                    fuzzer_bin,
                    "-minimize_crash=1",
                    "-runs=1000000",
                    f"-exact_artifact_path={exact}",
                    str(source),
                ],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=timeout_seconds,
                check=False,
            )
            record["minimize_returncode"] = completed.returncode
            record["minimize_tail"] = (completed.stderr + completed.stdout)[-2000:]
        except (OSError, subprocess.TimeoutExpired) as error:
            record["minimize_error"] = str(error)[:500]
        # A partially minimized crasher libFuzzer already wrote is still
        # hashed and kept even when the run itself timed out or errored.
        if exact.is_file():
            record["minimized_sha256"] = hashlib.sha256(exact.read_bytes()).hexdigest()
            record["minimized_size_bytes"] = exact.stat().st_size
            (artifacts_dir / f"minimized-{record['minimized_sha256']}").write_bytes(
                exact.read_bytes()
            )
        out.append(record)
    return out


def parse_coverage(output: str) -> dict[str, object]:
    """Coverage signals from libFuzzer output (repair round 7c).

    The hard proof is the inline 8-bit counter total from the Loaded
    line; ft at INITED/DONE and NEW events corroborate. When startup
    lines are truncated from the tail, counters is None and the caller
    falls back to ft presence.
    """
    import re

    counters: int | None = None
    match = re.search(r"Loaded \d+ modules\s+\(([\d,]+) inline 8-bit counters\)", output)
    if match:
        counters = int(match.group(1).replace(",", ""))
    inited = re.search(r"#\d+\s+INITED\b[^\n]*ft:\s*(\d+)", output)
    done = re.search(r"#\d+\s+DONE\b[^\n]*ft:\s*(\d+)", output)
    return {
        "counters": counters,
        "ft_inited": int(inited.group(1)) if inited else None,
        "ft_done": int(done.group(1)) if done else None,
        "new_events": bool(re.search(r"#\d+\s+NEW\b", output)),
    }


def parse_executed_units(stderr: str) -> int:
    """Units libFuzzer reports executing, or -1 when the line is absent."""
    import re

    matches = re.findall(r"Done (\d+) runs", stderr)
    return int(matches[-1]) if matches else -1


def owner_lib_sancov_symbols() -> dict[str, int]:
    """sanitizer_cov symbol counts per sley-* rlib in this slice's target dir.

    Proof that coverage instrumentation reaches the owner library code, not
    just the fuzz binary crate: with the old bin-only -Cpasses flag the
    owner rlibs carry zero sanitizer_cov symbols. Only the newest rlib per
    crate counts: the persistent target dir accumulates every build's
    rlibs, so summing all versions would let a stale instrumented rlib
    satisfy the gate after an instrumentation regression (fail-open on a
    warm host).
    """
    nm = shutil.which("llvm-nm") or shutil.which("nm")
    counts: dict[str, int] = {}
    if nm is None:
        return counts
    newest: dict[str, tuple[float, Path]] = {}
    for rlib in sorted((TARGET_DIR / "release" / "deps").glob("libsley_*.rlib")):
        try:
            mtime = rlib.stat().st_mtime
        except OSError:
            continue
        crate = rlib.name.split("-")[0]
        if crate not in newest or mtime > newest[crate][0]:
            newest[crate] = (mtime, rlib)
    for crate in sorted(newest):
        try:
            completed = subprocess.run(
                [nm, str(newest[crate][1])],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=300,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired):
            continue
        counts[newest[crate][1].name] = sum(
            1 for line in completed.stdout.splitlines() if "sanitizer_cov" in line
        )
    return counts


# Without a sanitizer runtime libFuzzer prints three WARNING lines on
# every run. They are expected for sancov-only builds; anything else
# WARNING-shaped is recorded and fails the run.
KNOWN_BENIGN_WARNINGS = (
    'WARNING: Failed to find function "__sanitizer_acquire_crash_state".',
    'WARNING: Failed to find function "__sanitizer_print_stack_trace".',
    'WARNING: Failed to find function "__sanitizer_set_death_callback".',
)


def unexpected_warnings(output: str) -> list[str]:
    """WARNING lines outside the documented sancov-only allowlist."""
    return sorted(
        {
            line
            for line in output.splitlines()
            if line.startswith("WARNING:") and line not in KNOWN_BENIGN_WARNINGS
        }
    )


def toolchain_versions() -> dict[str, str]:
    """Resolved toolchain versions for the proof record (round 7e).

    Records what actually built and ran: the resolved C compiler and the
    pinned Rust toolchain. A qualification default with no recorded proof
    stays visible as such instead of implied.
    """
    versions: dict[str, str] = {}
    commands = (
        ("cc", [CC, "--version"]),
        ("rust", ["rustup", "run", RUST_TOOLCHAIN, "rustc", "--version"]),
    )
    for label, argv in commands:
        try:
            completed = subprocess.run(
                argv,
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )
        except OSError as error:
            versions[label] = f"unavailable:{error}"
            continue
        versions[label] = (
            completed.stdout.strip().splitlines()[0]
            if completed.returncode == 0 and completed.stdout.strip()
            else f"unavailable:{completed.returncode}"
        )
    return versions


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
            "stdout": output_tail(completed.stdout),
            "stderr": output_tail(completed.stderr),
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
    """Newest 4000 chars plus oldest 2000: libFuzzer's startup lines carry
    the Loaded-modules counter total, so a tail-only window degrades the
    coverage proof silently once output grows."""
    if value is None:
        return ""
    if isinstance(value, bytes):
        value = value.decode("utf-8", errors="replace")
    if len(value) <= 6000:
        return value
    return value[:2000] + "\n[...middle truncated...]\n" + value[-4000:]


def write_evidence(evidence: dict[str, object]) -> None:
    EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print(json.dumps(evidence, indent=2, sort_keys=True))


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Build and run the S20-300 full complete-root snapshot decoder persistent libFuzzer target."""

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
RUNTIME = ROOT / "evidence/runtime/s20-700-complete-root-snapshot-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/complete_root_snapshot_decoder"
FIXTURE = ROOT / "conformance/complete-root-index-snapshot/v1/accepted.json"
REJECTED = ROOT / "conformance/complete-root-index-snapshot/v1/rejected.json"
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
OWNER_RLIB = "libsley_query-"
MAX_PAYLOAD_LEN = 65_536
MAX_LEN = MAX_PAYLOAD_LEN + 1
SMOKE_RUNS = 960
SMOKE_TIMEOUT_SECONDS = 60
SELECTOR_COUNT = 2


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
    # The coverage floor is measured on the on-disk corpus files, not the
    # generated seed count: libFuzzer adds inputs every run, so a floor on
    # seeds alone decays back to seed replay. The run must cover the corpus,
    # so the floor guarantees the whole on-disk corpus plus headroom for
    # genuine mutation on every smoke, including explicit small --runs.
    corpus_file_count = sum(1 for entry in CORPUS.iterdir() if entry.is_file())
    runs_floor = max(args.runs, corpus_file_count + 256)

    evidence: dict[str, object] = {
        "contract": "s20-700-complete-root-snapshot-decoder-persistent-libfuzzer-slice-v1",
        "scope": "COMPLETE_ROOT_SNAPSHOT_DECODER_ONLY",
        "full_s20_700_complete": False,
        "selector_count": SELECTOR_COUNT,
        "max_payload_bytes": MAX_PAYLOAD_LEN,
        "corpus_count": corpus_count,
        "seed_source": str(FIXTURE.relative_to(ROOT)),
        "runtime_path": str(RUNTIME.relative_to(ROOT)),
        "commands": [],
        "source_commit": git_output(["git", "rev-parse", "HEAD"]),
        "worktree_dirty": bool(git_output(["git", "status", "--porcelain"])),
        "toolchain_versions": toolchain_versions(),
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
            "complete_root_snapshot_decoder",
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
            # Coverage instrumentation reaches every crate through the
            # target rustflags above; only the link arguments stay here.
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
    evidence["owner_lib_sancov"] = sum(
        count
        for name, count in evidence["owner_lib_sancov_symbols"].items()
        if name.startswith(OWNER_RLIB)
    )
    if evidence["owner_lib_sancov_total"] == 0 or evidence["owner_lib_sancov"] == 0:
        evidence["result"] = "FAIL"
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
        evidence.setdefault("problems", []).append(
            f"owner-lib-sancov-missing:{OWNER_RLIB}"
        )
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 1

    if args.manual:
        command = [str(FUZZER), f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)]
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    fuzz = run(
        [str(FUZZER), f"-runs={runs_floor}", "-len_control=0", f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)],
        timeout=args.timeout,
    )
    evidence["commands"].append(fuzz)
    evidence["libfuzzer_output_tail"] = (fuzz["stderr"] + fuzz["stdout"])[-4000:]
    evidence["duration_seconds"] = round(time.monotonic() - started, 3)
    fuzz_output = str(fuzz["stderr"] + fuzz["stdout"])
    evidence["executed_runs"] = parse_executed_units(str(fuzz["stderr"]))
    evidence["runs_floor"] = runs_floor
    evidence["corpus_file_count"] = corpus_file_count
    evidence["coverage"] = parse_coverage(fuzz_output)
    coverage_counters = evidence["coverage"]["counters"]
    evidence["coverage_ok"] = (
        coverage_counters > 0
        if isinstance(coverage_counters, int)
        else ("ft:" in fuzz_output)
    )
    evidence["crash_artifacts"] = crash_artifact_names(ARTIFACTS)
    evidence["minimized_crashes"] = (
        minimize_crashes(
            fuzzer_bin=str(FUZZER),
            artifacts_dir=ARTIFACTS,
            minimized_dir=RUNTIME / "minimized",
            timeout_seconds=args.timeout,
        )
        if evidence["crash_artifacts"]
        else []
    )
    evidence["unexpected_warnings"] = unexpected_warnings(fuzz_output)
    if (
        fuzz["returncode"] == 0
        and evidence["executed_runs"] >= runs_floor
        and evidence["coverage_ok"]
        and not evidence["crash_artifacts"]
        and not evidence["unexpected_warnings"]
    ):
        evidence["result"] = "PASS"
    else:
        evidence["result"] = "FAIL"
        evidence.setdefault("problems", []).append(
            f"executed {evidence['executed_runs']} of floor {runs_floor} "
            f"(coverage={evidence['coverage']}, "
            f"crashes={evidence['crash_artifacts']}, warnings={evidence['unexpected_warnings']})"
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


def generate_seed_corpus() -> tuple[int, int]:
    CORPUS.mkdir(parents=True, exist_ok=True)

    accepted = json.loads(FIXTURE.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    if accepted.get("contract") != "sley2-complete-root-index-snapshot-v1":
        raise SystemExit("complete-root index snapshot fixture contract drifted")
    records = [bytes.fromhex(vector["record_hex"]) for vector in accepted["vectors"]]
    for vector, record in zip(accepted["vectors"], records):
        if len(record) != vector.get("record_bytes") or not record.startswith(b"SLEYIDX1"):
            raise SystemExit("complete-root index snapshot fixture structure drifted")
    payloads = list(records)
    payloads.extend(bytes.fromhex(mutation["candidate_hex"]) for mutation in rejected["mutations"])
    canonical = records[-1]
    payloads.extend([canonical + b"\x00", canonical + b"\xff", b"SLEYIDX1"])
    boundaries = {1, 8, 16, 48, 84, 88, 120, 124, 147, 148, len(canonical) - 33, len(canonical) - 32, len(canonical) - 1}
    boundaries.update(range(132, len(canonical), 64))
    payloads.extend(canonical[:length] for length in sorted(boundaries) if 0 < length < len(canonical))
    for seed in range(128):
        value = bytearray(canonical)
        index = (seed * 2_654_435_761) % len(value)
        value[index] ^= 1 << (seed % 8)
        payloads.append(bytes(value))

    seeds = [bytes([selector]) + payload for selector in range(SELECTOR_COUNT) for payload in payloads]
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


if __name__ == "__main__":
    sys.exit(main())

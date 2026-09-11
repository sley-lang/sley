#!/usr/bin/env python3
"""Build and run the S20-520 merge conflict-decoder and ancestor-rule persistent libFuzzer target."""

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
RUNTIME = ROOT / "evidence/runtime/s20-700-merge-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/merge_conflict_decoder"
FIXTURE = ROOT / "conformance/merge/v1/accepted.json"
REJECTED = ROOT / "conformance/merge/v1/rejected.json"
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
OWNER_RLIB = "libsley_repo-"
MAX_PAYLOAD_LEN = 65_536
MAX_LEN = MAX_PAYLOAD_LEN + 1
SMOKE_RUNS = 638
SMOKE_TIMEOUT_SECONDS = 60
SELECTOR_COUNT = 3


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
        "contract": "s20-700-merge-persistent-libfuzzer-slice-v1",
        "scope": "MERGE_CONFLICT_DECODER_AND_ANCESTOR_RULE",
        "section_18_5_surface": "merge engine",
        "full_s20_700_complete": False,
        "selector_count": SELECTOR_COUNT,
        "max_payload_bytes": MAX_PAYLOAD_LEN,
        "corpus_count": corpus_count,
        "seed_source": str(FIXTURE.relative_to(ROOT)),
        "runtime_path": str(RUNTIME.relative_to(ROOT)),
        "commands": [],
        "source_commit": git_output(["git", "rev-parse", "HEAD"]),
        "worktree_dirty": bool(git_output(["git", "status", "--porcelain"])),
        "worktree_dirty_files": git_output(["git", "status", "--porcelain"]).splitlines()[:50],
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
            "merge_conflict_decoder",
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
    evidence.update(derived_build_provenance(build))
    if build["returncode"] != 0:
        evidence["result"] = "FAIL"
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 1

    evidence["owner_lib_sancov_symbols"], evidence["rlib_linkage"] = (
        owner_lib_sancov_symbols(build_record=build)
    )
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

    prior_crashes = crash_artifact_names(ARTIFACTS)
    if args.manual:
        command = [str(FUZZER), f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)]
        print(" ".join(command))
        return subprocess.call(command, cwd=ROOT)

    fuzz = run(
        [str(FUZZER), f"-runs={runs_floor}", "-len_control=0", "-timeout=30", "-rss_limit_mb=2048", f"-max_len={MAX_LEN}", f"-artifact_prefix={ARTIFACTS}/", str(CORPUS)],
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
    ft_inited = evidence["coverage"]["ft_inited"]
    ft_done = evidence["coverage"]["ft_done"]
    # Strict monotonic feature gate (repair round 7h): counters prove
    # instrumentation, and ft_done >= ft_inited > 0 proves the run
    # executed and grew coverage accounting. Saturated corpora pass
    # with equality; a truncated or missing INITED/DONE line fails
    # instead of degrading to substring presence.
    evidence["coverage_ok"] = (
        isinstance(coverage_counters, int)
        and coverage_counters > 0
        and isinstance(ft_inited, int)
        and isinstance(ft_done, int)
        and ft_done >= ft_inited
        and ft_done > 0
    )
    evidence["crash_artifacts"] = crash_artifact_names(ARTIFACTS)
    evidence["retested_prior_crashes"] = retest_prior_crashes(
        fuzzer_bin=str(FUZZER),
        artifacts_dir=ARTIFACTS,
        prior=prior_crashes,
        timeout_seconds=args.timeout,
    )
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
    evidence["unexpected_warnings"] = [
            line
            for line in fuzz["warnings"]
            if line not in KNOWN_BENIGN_WARNINGS
        ]
    if (
        fuzz["returncode"] == 0
        and evidence["executed_runs"] >= runs_floor
        and evidence["coverage_ok"]
        and not evidence["crash_artifacts"]
        and not any(
            record.get("still_crashes", False)
            for record in evidence["retested_prior_crashes"]
        )
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


def retest_prior_crashes(
    *, fuzzer_bin: str, artifacts_dir: Path, prior: list[str], timeout_seconds: int
) -> list[dict[str, object]]:
    """Re-execute previous runs' crash artifacts (crash-to-regression).

    Repair round 7h: a fixed crasher must stay fixed. Each prior artifact
    runs once against the current binary in an isolated retest dir (never
    the shared artifacts dir, so retest crashes cannot pollute the
    record). A still-crashing prior fails the run; a cleared one is
    noted, never silently dropped.
    """
    if not prior:
        return []
    retest_dir = artifacts_dir / "retest-tmp"
    if retest_dir.exists():
        shutil.rmtree(retest_dir)
    retest_dir.mkdir(parents=True)
    out: list[dict[str, object]] = []
    for name in sorted(prior):
        source = artifacts_dir / name
        record: dict[str, object] = {"artifact": name}
        if not source.is_file():
            record["still_crashes"] = False
            record["note"] = "artifact cleared by owner triage"
            out.append(record)
            continue
        try:
            completed = subprocess.run(
                [
                    fuzzer_bin,
                    "-runs=1",
                    "-timeout=30",
                    "-rss_limit_mb=2048",
                    f"-artifact_prefix={retest_dir}/",
                    str(source),
                ],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=min(timeout_seconds, 120),
                check=False,
            )
            record["still_crashes"] = completed.returncode != 0
            record["returncode"] = completed.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            record["still_crashes"] = True
            record["retest_error"] = str(error)[:200]
        out.append(record)
    shutil.rmtree(retest_dir, ignore_errors=True)
    return out


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
                    f"-max_total_time={max(10, timeout_seconds - 15)}",
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
    line; ft at INITED/DONE and NEW events corroborate. The caller gates strictly on counters and monotonic ft; no
    silent fallback exists. libFuzzer WARNINGs are captured separately
    from the full streams (see warning_lines).
    """
    import re

    counters: int | None = None
    match = re.search(r"Loaded \d+ modules\s+\(([\d,]+) inline 8-bit counters\)", output)
    if match:
        counters = int(match.group(1).replace(",", ""))
    inited = re.search(r"#\d+\s+INITED\b[^\n]*ft:\s*(\d+)", output)
    done = re.search(r"#\d+\s+DONE\b[^\n]*ft:\s*(\d+)", output)
    seed = re.search(r"INFO: Seed:\s*(\d+)", output)
    return {
        "counters": counters,
        "ft_inited": int(inited.group(1)) if inited else None,
        "ft_done": int(done.group(1)) if done else None,
        "new_events": bool(re.search(r"#\d+\s+NEW\b", output)),
        "seed": int(seed.group(1)) if seed else None,
    }


def parse_executed_units(stderr: str) -> int:
    """Units libFuzzer reports executing, or -1 when the line is absent."""
    import re

    matches = re.findall(r"Done (\d+) runs", stderr)
    return int(matches[-1]) if matches else -1


def owner_lib_sancov_symbols(*, build_record: dict | None = None) -> tuple[dict[str, int], str]:
    """sanitizer_cov symbol counts for the rlibs cargo linked into the binary.

    Proof that coverage instrumentation reaches the owner library code, not
    just the fuzz binary crate: with the old bin-only -Cpasses flag the
    owner rlibs carry zero sanitizer_cov symbols. The rlib paths come from
    cargo itself (--message-format=json over the exact build argv, fresh
    and fast), so the gate counts what the binary linked, never a stale
    rlib that happens to share the persistent target dir. When the cargo
    query fails, the gate falls back to the newest rlib per crate and
    says so in the linkage method.
    """
    rlibs, method = linked_sley_rlibs(build_record)
    nm = shutil.which("llvm-nm") or shutil.which("nm")
    counts: dict[str, int] = {}
    if nm is None:
        return counts, "nm-missing"
    for rlib in rlibs:
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
    return counts, method


def linked_sley_rlibs(build_record: dict | None) -> tuple[list[Path], str]:
    """rlib paths cargo reports for the recorded build, or newest-per-crate.

    The recorded build argv is replayed with --message-format=json (no
    recompilation when fresh, seconds): compiler-artifact filenames are
    fingerprint-authoritative, unlike mtime globbing.
    """
    if build_record is not None:
        argv = [str(arg) for arg in build_record.get("argv", [])]
        if "--" in argv:
            # The full argv is replayed (link arguments included): fresh
            # artifacts report in seconds without recompiling, and a
            # non-fresh tree rebuilds identically instead of failing.
            query = (
                argv[: argv.index("--")]
                + ["--message-format=json"]
                + argv[argv.index("--") :]
            )
            try:
                completed = subprocess.run(
                    query,
                    cwd=ROOT,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=600,
                    check=False,
                )
            except (OSError, subprocess.TimeoutExpired):
                completed = None
            if completed is not None and completed.returncode == 0:
                rlibs = []
                for line in completed.stdout.splitlines():
                    try:
                        message = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if message.get("reason") != "compiler-artifact":
                        continue
                    for filename in message.get("filenames", []):
                        if "/deps/libsley_" in filename and filename.endswith(".rlib"):
                            rlibs.append(Path(filename))
                if rlibs:
                    return sorted(set(rlibs)), "cargo-json"
    newest: dict[str, tuple[float, Path]] = {}
    for rlib in sorted((TARGET_DIR / "release" / "deps").glob("libsley_*.rlib")):
        try:
            mtime = rlib.stat().st_mtime
        except OSError:
            continue
        crate = rlib.name.split("-")[0]
        if crate not in newest or mtime > newest[crate][0]:
            newest[crate] = (mtime, rlib)
    return [path for _, path in sorted(newest.values())], "mtime-fallback"
# Without a sanitizer runtime libFuzzer prints three WARNING lines on
# every run. They are expected for sancov-only builds; anything else
# WARNING-shaped is recorded and fails the run.
KNOWN_BENIGN_WARNINGS = (
    'WARNING: Failed to find function "__sanitizer_acquire_crash_state".',
    'WARNING: Failed to find function "__sanitizer_print_stack_trace".',
    'WARNING: Failed to find function "__sanitizer_set_death_callback".',
)


def warning_lines(stdout: str | bytes | None, stderr: str | bytes | None) -> list[str]:
    """Every WARNING-shaped line in the full streams (capped at 50).

    Repair round 7i: libFuzzer WARNINGs are startup/teardown lines, but
    the gate must not depend on truncation windows, so run() captures
    them from the full output before truncating. Expected sancov-only
    lines are filtered by the caller against KNOWN_BENIGN_WARNINGS.
    """
    text = ""
    for value in (stdout, stderr):
        if isinstance(value, bytes):
            value = value.decode("utf-8", errors="replace")
        if value:
            text += value + "\n"
    return sorted({line for line in text.splitlines() if line.startswith("WARNING:")})[:50]




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


def derived_build_provenance(build_record: dict) -> dict[str, object]:
    """Build provenance derived from the executed build command, not labels.

    Repair round 7h: build_locked and sancov_scope describe the argv that
    actually ran (recorded in the build entry), so a flag regression
    changes the record instead of passing under a stale label.
    """
    argv = [str(arg) for arg in build_record.get("argv", [])]
    return {
        "build_locked": "--locked" in argv,
        "sancov_scope": (
            "workspace-target-units-via-host-config"
            if any("target.x86_64-unknown-linux-gnu.rustflags" in arg for arg in argv)
            else "unknown"
        ),
    }


def toolchain_problems() -> list[str]:
    problems = []
    for poisoned in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"):
        if poisoned in os.environ:
            problems.append(
                f"{poisoned}-set-in-environment: unset it so the host-config "
                "sancov flags are the ones that build"
            )
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
    if accepted.get("contract") != "sley2-merge-v1":
        raise SystemExit("merge fixture contract drifted")
    stored = [
        bytes.fromhex(vector["expected"]["stored_hex"])
        for vector in accepted["vectors"]
        if vector.get("outcome") == "conflict"
    ]
    if not stored:
        raise SystemExit("merge fixture carries no conflict vector")
    payloads = list(stored)
    payloads.extend(bytes.fromhex(mutation["input_hex"]) for mutation in rejected["mutations"])
    canonical = stored[0]
    payloads.extend([canonical + b"\x00", canonical + b"\xff", b"SLEYSCB1"])
    boundaries = {1, 8, 9, 10, len(canonical) - 33, len(canonical) - 32, len(canonical) - 1}
    boundaries.update(range(16, len(canonical), 64))
    payloads.extend(canonical[:length] for length in sorted(boundaries) if length > 0)
    for seed in range(128):
        value = bytearray(canonical)
        index = (seed * 2_654_435_761) % len(value)
        value[index] ^= 1 << (seed % 8)
        payloads.append(bytes(value))
    decoder_seeds = [bytes([selector]) + payload for selector in (0, 1) for payload in payloads]
    ancestor_seeds = [
        bytes([2]) + bytes([5, 4, 2, 1, 0xFF, 7, 6, 2, 1]),
        bytes([2]) + bytes([9, 0xFF, 1]),
        bytes([2]) + bytes([3, 2, 1]),
        bytes([2]) + bytes([0xFF]),
        bytes([2]) + bytes(range(1, 64)) + bytes([0xFF]) + bytes(range(40, 80)),
    ]
    unique_seeds = list(dict.fromkeys(decoder_seeds + ancestor_seeds))
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
            "warnings": warning_lines(completed.stdout, completed.stderr),
        }
    except subprocess.TimeoutExpired as error:
        return {
            "argv": command,
            "returncode": 124,
            "duration_seconds": round(time.monotonic() - started, 3),
            "stdout": output_tail(error.stdout),
            "stderr": output_tail(error.stderr),
            "warnings": warning_lines(error.stdout, error.stderr),
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

#!/usr/bin/env python3
"""Build and run the S20-420 SMP1 JSON bridge persistent libFuzzer target."""

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
RUNTIME = ROOT / "evidence/runtime/s20-700-smp1-json-bridge-libfuzzer"
CORPUS = RUNTIME / "corpus"
ARTIFACTS = RUNTIME / "artifacts"
EVIDENCE = RUNTIME / "evidence.json"
TARGET_DIR = RUNTIME / "target"
FUZZER = TARGET_DIR / "release/smp1_json_bridge"
FIXTURE = ROOT / "conformance/smp1-json-bridge/v1/roundtrip.json"
REJECTED = ROOT / "conformance/smp1-json-bridge/v1/rejected.json"
TABLE = ROOT / "conformance/smp1-json-bridge/v1/methods.json"
CLANG_VERSION = "18.1.8"
RUST_TOOLCHAIN = "nightly-2026-02-27"
# Canonical LLVM-18 layouts (pin-layout repair): the official Debian-style
# and Arch-style package layouts. Compiler and runtime must pair within one
# layout; explicit paths only, no PATH fallback beyond the Debian layout's
# own clang-18 name, no symlinks, no ambient toolchain. SLEY_FUZZ_CC and
# SLEY_FUZZ_LIBFUZZER_A let an owner lane prove the harness on an equivalent
# toolchain without editing it; such runs resolve as layout "override"
# (noncanonical) and the resolved values land in evidence.json.
CANONICAL_LAYOUTS = (
    (
        "debian",
        "clang-18",
        Path("/usr/lib/llvm-18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a"),
    ),
    (
        "arch",
        "/usr/lib/llvm18/bin/clang-18",
        Path("/usr/lib/llvm18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a"),
    ),
)
CC_ENV = os.environ.get("SLEY_FUZZ_CC")
FUZZER_RT_ENV = os.environ.get("SLEY_FUZZ_LIBFUZZER_A")


def resolve_fuzz_toolchain() -> tuple[str, Path, str]:
    """Resolve (compiler, runtime, layout) from the canonical layouts.

    Any explicit SLEY_FUZZ_* override resolves as layout "override". Else
    the first layout whose compiler and runtime both exist wins; if no
    layout pairs up, resolve the first-layout defaults as "missing".
    """
    if CC_ENV is not None or FUZZER_RT_ENV is not None:
        return (
            CC_ENV or CANONICAL_LAYOUTS[0][1],
            Path(FUZZER_RT_ENV) if FUZZER_RT_ENV else CANONICAL_LAYOUTS[0][2],
            "override",
        )
    for layout, cc, rt in CANONICAL_LAYOUTS:
        if layout == "debian":
            if shutil.which(cc) is not None and rt.exists():
                return cc, rt, layout
        elif Path(cc).is_file() and rt.exists():
            return cc, rt, layout
    return CANONICAL_LAYOUTS[0][1], CANONICAL_LAYOUTS[0][2], "missing"


def clang_version_string(cc: str) -> str:
    """First line of `<cc> --version`, or unavailable:<reason>."""
    try:
        completed = subprocess.run(
            [cc, "--version"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
    except OSError as error:
        return f"unavailable:{error}"
    if completed.returncode == 0 and completed.stdout.strip():
        return completed.stdout.strip().splitlines()[0]
    return f"unavailable:{completed.returncode}"


def libfuzzer_runtime_sha256(rt: Path) -> str:
    """SHA-256 of the linked libFuzzer runtime archive, or missing:<path>."""
    try:
        return hashlib.sha256(rt.read_bytes()).hexdigest()
    except OSError:
        return f"missing:{rt}"


CC, FUZZER_RT, FUZZ_LAYOUT = resolve_fuzz_toolchain()
BUILD_TIMEOUT_SECONDS = 1200
OWNER_RLIB = "libsley_json_bridge-"
MAX_PAYLOAD_LEN = 65_536
MAX_LEN = MAX_PAYLOAD_LEN + 1
SMOKE_RUNS = 1286
SMOKE_TIMEOUT_SECONDS = 90
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
        "contract": "s20-700-smp1-json-bridge-persistent-libfuzzer-slice-v1",
        "scope": "SMP1_JSON_BRIDGE_TEXT_AND_FRAME_ROUND_TRIP_ONLY",
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
    evidence.setdefault("toolchain_overridden", FUZZ_LAYOUT == "override")
    evidence.setdefault("cc", CC)
    evidence.setdefault("libfuzzer_runtime", str(FUZZER_RT))
    evidence.setdefault("fuzz_toolchain_layout", FUZZ_LAYOUT)
    evidence.setdefault("fuzz_toolchain_version", clang_version_string(CC))
    evidence.setdefault("libfuzzer_runtime_sha256", libfuzzer_runtime_sha256(FUZZER_RT))
    evidence.setdefault("stale_seeds_removed", stale_seeds_removed)
    if evidence["problems"]:
        evidence["result"] = "BLOCKED"
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
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
            "smp1_json_bridge",
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
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
        EVIDENCE.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        print(json.dumps(evidence, indent=2, sort_keys=True))
        return 1

    evidence["owner_lib_sancov_symbols"], evidence["rlib_linkage"] = (
        owner_lib_sancov_symbols(build_record=build, env=env)
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
        evidence["duration_seconds"] = round(time.monotonic() - started, 3)
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
    prior_set = set(prior_crashes)
    evidence["new_crash_artifacts"] = [
        name for name in evidence["crash_artifacts"] if name not in prior_set
    ]
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
            skip=[
                record["artifact"]
                for record in evidence["retested_prior_crashes"]
                if not record.get("still_crashes", False)
            ],
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
        and not evidence["new_crash_artifacts"]
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
            f"new_crashes={evidence['new_crash_artifacts']} "
            f"still_crashing={[r['artifact'] for r in evidence['retested_prior_crashes'] if r.get('still_crashes')]} "
            f"warnings={evidence['unexpected_warnings']})"        )
    evidence["duration_seconds"] = round(time.monotonic() - started, 3)
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
    skip: list[str] | None = None,
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
    skipped = set(skip or [])
    for name in crash_artifact_names(artifacts_dir):
        if name in skipped:
            continue
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


def owner_lib_sancov_symbols(*, build_record: dict | None = None, env: dict[str, str] | None = None) -> tuple[dict[str, int], str]:
    """sanitizer_cov symbol counts for the rlibs cargo linked into the binary.

    Proof that coverage instrumentation reaches the owner library code, not
    just the fuzz binary crate: with the old bin-only -Cpasses flag the
    owner rlibs carry zero sanitizer_cov symbols. The rlib paths come from
    cargo itself (--message-format=json over the exact build argv, fresh
    and fast), so the gate counts what the binary linked, never a stale
    rlib that happens to share the persistent target dir. When the cargo
    query fails, the gate counts nothing (method cargo-json-failed) and
    fails closed instead of passing on unknown provenance.
    """
    rlibs, method = linked_sley_rlibs(build_record, env)
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


def linked_sley_rlibs(build_record: dict | None, env: dict[str, str] | None) -> tuple[list[Path], str]:
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
            env = dict(os.environ) if env is None else env
            try:
                completed = subprocess.run(
                    query,
                    cwd=ROOT,
                    env=env,
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
    # No mtime fallback by design: newest-per-crate globbing can count a
    # stale rlib that shares the persistent target dir, so a failed cargo
    # query fails the gate instead of passing on unknown provenance.
    return [], "cargo-json-failed"
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
    if FUZZ_LAYOUT == "override":
        if shutil.which(CC) is None and not Path(CC).is_file():
            problems.append(f"{CC}-missing")
    elif FUZZ_LAYOUT == "missing":
        problems.append("canonical-clang-18-missing")
    else:
        version = clang_version_string(CC)
        if not version.startswith(f"clang version {CLANG_VERSION}"):
            problems.append(f"clang-version-mismatch:{version}")
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
    table = json.loads(TABLE.read_text(encoding="utf-8"))
    if accepted.get("contract") != "sley2-smp1-json-bridge-v1":
        raise SystemExit("SMP1 JSON bridge fixture contract drifted")
    texts = [vector["json"].encode("utf-8") for vector in accepted["vectors"]]
    frames = [bytes.fromhex(vector["frame_hex"]) for vector in accepted["vectors"]]
    for frame in frames:
        if frame[8:16] != b"SLEYSCB1":
            raise SystemExit("SMP1 JSON bridge fixture structure drifted")
    texts.extend(mutation["json"].encode("utf-8") for mutation in rejected["mutations"])
    names = [row["name"] for row in table["methods"]]
    hello = {
        "adapters": ["a1" * 32, "a2" * 32],
        "effects": ["e1" * 32],
        "features": {"cancel": True, "checksum": False, "json_bridge": True, "stream": True},
        "limits": {
            "max_depth": 16,
            "max_edges": 10_000,
            "max_entities": 1_000,
            "max_frame_bytes": 1_048_576,
            "max_inflight": 4,
            "max_sessions": 16,
            "max_response_bytes": 1_048_576,
            "max_work": 1_000_000,
        },
        "methods": names,
        "protocol_versions": [1, 2],
        "schema_epochs": ["11" * 32, "12" * 32],
    }
    records = [
        json.dumps(hello, sort_keys=True, separators=(",", ":")).encode(),
        b'{"bytes":"00","index":0,"total":1}',
        b'{"code":40009,"details":"","incident":null,"phase":0,"retryability":"never","symbol":"PROTOCOL_LIMIT_EXCEEDED"}',
        b'{"code":"9007199254740992","details":"ff","incident":"' + b"ab" * 32 + b'","phase":1,"retryability":"after_requery","symbol":"QUERY_CURSOR_INVALID"}',
    ]

    def derive(canonical: bytes) -> list[bytes]:
        derived = [canonical + b"\x00", canonical + b"\xff", canonical[:1], b""]
        boundaries = {1, 8, 9, 16, 17, 18, 50, 51, 52, len(canonical) - 33, len(canonical) - 32, len(canonical) - 1}
        boundaries.update(range(20, len(canonical), 16))
        derived.extend(canonical[:length] for length in sorted(boundaries) if 0 < length < len(canonical))
        for seed in range(128):
            value = bytearray(canonical)
            index = (seed * 2_654_435_761) % len(value)
            value[index] ^= 1 << (seed % 8)
            derived.append(bytes(value))
        return derived

    lanes = {
        0: texts + records + derive(texts[0]),
        1: frames + derive(frames[0]),
        2: records + texts + derive(records[0]),
    }
    seeds = [bytes([selector]) + payload for selector, payloads in lanes.items() for payload in payloads]
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
